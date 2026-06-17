//! XML document model and serialiser.
//!
//! The serialiser reproduces the layout of the reference decompiler: two space
//! indentation, self closing empty elements, and attributes rendered inline.

/// Output text encoding for the rendered document.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Encoding {
    /// UTF-8 output. This is the default and is always lossless.
    Utf8,
    /// Windows-1252 output, matching the encoding the simulator's own tools use.
    Windows1252,
}

impl Encoding {
    fn declaration(self) -> &'static str {
        match self {
            Encoding::Utf8 => "<?xml version=\"1.0\" encoding=\"utf-8\"?>",
            Encoding::Windows1252 => "<?xml version=\"1.0\" encoding=\"Windows-1252\"?>",
        }
    }

    fn encode(self, text: &str) -> Vec<u8> {
        match self {
            Encoding::Utf8 => text.as_bytes().to_vec(),
            Encoding::Windows1252 => encode_windows_1252(text),
        }
    }
}

/// A single XML element with optional attributes, child elements and text.
pub struct Node {
    pub name: String,
    pub attributes: Vec<(String, String)>,
    pub children: Vec<Node>,
    pub text: Option<String>,
}

impl Node {
    pub fn new(name: impl Into<String>) -> Self {
        Node {
            name: name.into(),
            attributes: Vec::new(),
            children: Vec::new(),
            text: None,
        }
    }
}

/// Serialise a document rooted at `root` into bytes using `encoding`.
pub fn render(root: &Node, encoding: Encoding) -> Vec<u8> {
    let mut body = String::new();
    render_node(root, "", &mut body);
    let document = format!("{}\n{}\n", encoding.declaration(), body);
    encoding.encode(&document)
}

fn render_node(node: &Node, indent: &str, out: &mut String) {
    let attributes: String = node
        .attributes
        .iter()
        .map(|(name, value)| format!(" {}=\"{}\"", name, escape(value)))
        .collect();

    let text_is_empty = node.text.as_deref().unwrap_or("").is_empty();
    if node.children.is_empty() && text_is_empty {
        out.push_str(&format!("{indent}<{}{} />", node.name, attributes));
        return;
    }
    if node.children.is_empty() {
        let text = escape(node.text.as_deref().unwrap_or(""));
        out.push_str(&format!(
            "{indent}<{}{}>{}</{}>",
            node.name, attributes, text, node.name
        ));
        return;
    }

    out.push_str(&format!("{indent}<{}{}>", node.name, attributes));
    let child_indent = format!("{indent}  ");
    for child in &node.children {
        out.push('\n');
        render_node(child, &child_indent, out);
    }
    out.push_str(&format!("\n{indent}</{}>", node.name));
}

fn escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(ch),
        }
    }
    out
}

fn encode_windows_1252(text: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len());
    for ch in text.chars() {
        let code = ch as u32;
        if code <= 0xFF {
            out.push(code as u8);
        } else {
            out.push(high_windows_1252(code).unwrap_or(b'?'));
        }
    }
    out
}

/// Reverse mapping for the printable characters Windows-1252 places in the
/// `0x80..=0x9F` range, which differ from Latin-1.
fn high_windows_1252(code: u32) -> Option<u8> {
    Some(match code {
        0x20AC => 0x80,
        0x201A => 0x82,
        0x0192 => 0x83,
        0x201E => 0x84,
        0x2026 => 0x85,
        0x2020 => 0x86,
        0x2021 => 0x87,
        0x02C6 => 0x88,
        0x2030 => 0x89,
        0x0160 => 0x8A,
        0x2039 => 0x8B,
        0x0152 => 0x8C,
        0x017D => 0x8E,
        0x2018 => 0x91,
        0x2019 => 0x92,
        0x201C => 0x93,
        0x201D => 0x94,
        0x2022 => 0x95,
        0x2013 => 0x96,
        0x2014 => 0x97,
        0x02DC => 0x98,
        0x2122 => 0x99,
        0x0161 => 0x9A,
        0x203A => 0x9B,
        0x0153 => 0x9C,
        0x017E => 0x9E,
        0x0178 => 0x9F,
        _ => return None,
    })
}
