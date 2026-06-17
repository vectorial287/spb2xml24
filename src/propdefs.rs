//! Loader for Microsoft Flight Simulator property definition files.
//!
//! Each compiled SPB tag references a property or set by GUID. The simulator
//! ships the GUID to name mapping as a tree of `propXXX.xml` symbol definition
//! files. This module scans that tree and builds a flat GUID keyed lookup.
//!
//! The scanner is deliberately tolerant: the definition files embed `cpptext`
//! blocks containing raw C++ that is not always well formed XML, so a streaming
//! tag scanner is used rather than a strict parser.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

/// What a GUID resolves to inside an SPB stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Type,
    Property,
    Set,
}

/// A resolved property, set or type definition.
#[derive(Clone, Debug)]
pub struct Def {
    pub kind: Kind,
    pub name: String,
    /// Name of the owning symbol, used to qualify element names.
    pub symbol: String,
    /// Value type for properties, for example `FLOAT` or `TEXT`.
    pub value_type: String,
    pub enum_values: Vec<String>,
    pub is_attribute: bool,
}

/// GUID keyed dictionary of every definition found under a propdefs directory.
pub struct Bank {
    by_guid: HashMap<String, Def>,
}

impl Bank {
    /// Load every `.xml` definition file under `dir`, recursively.
    pub fn load(dir: &Path) -> Result<Self> {
        if !dir.is_dir() {
            return Err(Error::Propdefs(format!(
                "propdefs directory not found: {}",
                dir.display()
            )));
        }

        let mut by_guid = HashMap::new();
        for file in xml_files(dir)? {
            let bytes = fs::read(&file)?;
            let text = strip_comments(&String::from_utf8_lossy(&bytes));
            let fallback = file
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            scan_file(&text, &fallback, &mut by_guid);
        }

        if by_guid.is_empty() {
            return Err(Error::Propdefs(format!(
                "no symbol definitions found under {}",
                dir.display()
            )));
        }
        Ok(Bank { by_guid })
    }

    /// Look up a definition by its normalised (brace free, lowercase) GUID.
    pub fn get(&self, guid: &str) -> Option<&Def> {
        self.by_guid.get(guid)
    }

    pub fn len(&self) -> usize {
        self.by_guid.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_guid.is_empty()
    }
}

fn scan_file(text: &str, fallback_symbol: &str, by_guid: &mut HashMap<String, Def>) {
    for (symbol_attrs, symbol_body) in find_blocks(text, "SymbolDef") {
        let attrs = parse_attrs(&symbol_attrs);
        let symbol = attr(&attrs, &["name", "Name"])
            .unwrap_or(fallback_symbol)
            .to_string();

        for (block_attrs, body) in find_blocks(&symbol_body, "TypeDef") {
            let attrs = parse_attrs(&block_attrs);
            let (Some(name), Some(id)) = (
                attr(&attrs, &["name", "Name"]),
                attr(&attrs, &["id", "ID", "Id"]),
            ) else {
                continue;
            };
            let value_type = binding_type(&body).unwrap_or_else(|| name.to_string());
            insert(
                by_guid,
                id,
                Def {
                    kind: Kind::Type,
                    name: name.to_string(),
                    symbol: symbol.clone(),
                    value_type,
                    enum_values: Vec::new(),
                    is_attribute: false,
                },
            );
        }

        for (block_attrs, body) in find_blocks(&symbol_body, "PropertyDef") {
            let attrs = parse_attrs(&block_attrs);
            let (Some(name), Some(id)) = (
                attr(&attrs, &["name", "Name"]),
                attr(&attrs, &["id", "ID", "Id"]),
            ) else {
                continue;
            };
            let value_type = attr(&attrs, &["type", "Type"]).unwrap_or("").to_string();
            let is_attribute = attr(&attrs, &["xml_io"])
                .map(|v| v.eq_ignore_ascii_case("attribute"))
                .unwrap_or(false);
            let enum_values = find_blocks(&body, "EnumVal")
                .into_iter()
                .map(|(enum_attrs, _)| {
                    attr(&parse_attrs(&enum_attrs), &["xml_name", "name", "Name"])
                        .unwrap_or("")
                        .to_string()
                })
                .collect();
            insert(
                by_guid,
                id,
                Def {
                    kind: Kind::Property,
                    name: name.to_string(),
                    symbol: symbol.clone(),
                    value_type,
                    enum_values,
                    is_attribute,
                },
            );
        }

        for (block_attrs, _) in find_blocks(&symbol_body, "SetDef") {
            let attrs = parse_attrs(&block_attrs);
            let (Some(name), Some(id)) = (
                attr(&attrs, &["name", "Name"]),
                attr(&attrs, &["id", "ID", "Id"]),
            ) else {
                continue;
            };
            insert(
                by_guid,
                id,
                Def {
                    kind: Kind::Set,
                    name: name.to_string(),
                    symbol: symbol.clone(),
                    value_type: String::new(),
                    enum_values: Vec::new(),
                    is_attribute: false,
                },
            );
        }
    }
}

fn insert(by_guid: &mut HashMap<String, Def>, id: &str, def: Def) {
    by_guid.entry(normalize_guid(id)).or_insert(def);
}

fn binding_type(type_def_body: &str) -> Option<String> {
    let (binding_attrs, _) = find_blocks(type_def_body, "binding").into_iter().next()?;
    attr(&parse_attrs(&binding_attrs), &["type", "Type"]).map(str::to_string)
}

fn normalize_guid(id: &str) -> String {
    id.trim()
        .trim_start_matches('{')
        .trim_end_matches('}')
        .to_ascii_lowercase()
}

fn xml_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        for entry in fs::read_dir(&current)? {
            let path = entry?.path();
            if path.is_dir() {
                stack.push(path);
            } else if path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("xml"))
            {
                files.push(path);
            }
        }
    }
    Ok(files)
}

fn strip_comments(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(start) = rest.find("<!--") {
        out.push_str(&rest[..start]);
        match rest[start + 4..].find("-->") {
            Some(end) => rest = &rest[start + 4 + end + 3..],
            None => return out,
        }
    }
    out.push_str(rest);
    out
}

/// Find every `<tag ...>...</tag>` (or self closing `<tag ... />`) at any depth,
/// returning the attribute text and inner body of each. Tags are not assumed to
/// nest within themselves, matching the structure of the definition files.
fn find_blocks(text: &str, tag: &str) -> Vec<(String, String)> {
    let mut blocks = Vec::new();
    let open = format!("<{tag}");
    let close = format!("</{tag}");
    let mut from = 0;

    while let Some(start) = find_token(text, &open, from) {
        let after_name = start + open.len();
        let Some(gt) = text[after_name..].find('>') else {
            break;
        };
        let tag_end = after_name + gt;
        let raw_attrs = &text[after_name..tag_end];
        let trimmed = raw_attrs.trim_end();

        if trimmed.ends_with('/') {
            let attrs = trimmed.trim_end_matches('/').to_string();
            blocks.push((attrs, String::new()));
            from = tag_end + 1;
            continue;
        }

        let attrs = trimmed.to_string();
        let body_start = tag_end + 1;
        match find_token(text, &close, body_start) {
            Some(close_at) => {
                let body = text[body_start..close_at].to_string();
                blocks.push((attrs, body));
                from = text[close_at..]
                    .find('>')
                    .map(|g| close_at + g + 1)
                    .unwrap_or(close_at + close.len());
            }
            None => {
                blocks.push((attrs, text[body_start..].to_string()));
                break;
            }
        }
    }
    blocks
}

/// Find `token` starting at or after `from`, requiring that the character after
/// the token is not part of a longer name (so `<TypeDef` does not match
/// `<TypeDefs`).
fn find_token(text: &str, token: &str, from: usize) -> Option<usize> {
    let mut search = from;
    while let Some(rel) = text[search..].find(token) {
        let at = search + rel;
        let after = at + token.len();
        match text[after..].chars().next() {
            Some(c) if c.is_alphanumeric() || c == '_' => search = after,
            _ => return Some(at),
        }
    }
    None
}

fn parse_attrs(input: &str) -> Vec<(String, String)> {
    let chars: Vec<char> = input.chars().collect();
    let mut attrs = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if !is_name_start(chars[i]) {
            i += 1;
            continue;
        }
        let start = i;
        i += 1;
        while i < chars.len() && is_name_part(chars[i]) {
            i += 1;
        }
        let name: String = chars[start..i].iter().collect();

        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }
        if i >= chars.len() || chars[i] != '=' {
            continue;
        }
        i += 1;
        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }
        if i >= chars.len() || (chars[i] != '"' && chars[i] != '\'') {
            continue;
        }
        let quote = chars[i];
        i += 1;
        let value_start = i;
        while i < chars.len() && chars[i] != quote {
            i += 1;
        }
        let value: String = chars[value_start..i].iter().collect();
        if i < chars.len() {
            i += 1;
        }
        attrs.push((name, value));
    }
    attrs
}

fn attr<'a>(attrs: &'a [(String, String)], names: &[&str]) -> Option<&'a str> {
    for wanted in names {
        for (key, value) in attrs {
            if key == wanted {
                return Some(value);
            }
        }
    }
    None
}

fn is_name_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

fn is_name_part(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | ':' | '.' | '-')
}
