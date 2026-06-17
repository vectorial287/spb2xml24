//! Parser for the compiled SPB property stream.
//!
//! Layout: a 16 bit file id, a twelve entry header (entry 6 is the tag count),
//! then `tag_count - 1` tag records. Each tag record is a 16 byte GUID plus a
//! 32 bit value size. The body is a depth first sequence of element tags that
//! reference those definitions by index, forming the document tree.
//!
//! The per tag value size (`aux` in the original format) is authoritative for
//! fixed size properties. In MSFS 2024 it distinguishes an "input pin" float,
//! stored as a GUID followed by one or more floats, from a plain float of the
//! same declared type. A size of `-1` marks a variable length value.

use crate::error::{format_err, Result};
use crate::format::{dec3, dec3_f32, lla};
use crate::guid;
use crate::propdefs::{Bank, Def, Kind};
use crate::reader::Reader;
use crate::textdecode::TextTable;
use crate::xml::Node;

const FILE_ID: u16 = 60332;
const HEADER_ENTRIES: usize = 12;
const TAG_COUNT_ENTRY: usize = 6;
const GUID_SIZE: i32 = 16;
const REVOLUTION: f64 = 4_294_967_296.0;

/// A resolved tag: its definition and on-disk value size.
struct Tag<'a> {
    def: &'a Def,
    size: i32,
}

/// Decode an SPB byte stream into an XML element tree.
pub fn parse(data: &[u8], bank: &Bank, text: &TextTable) -> Result<Node> {
    let mut reader = Reader::new(data);

    if reader.u16()? != FILE_ID {
        return Err(format_err!("not an SPB file: bad file id"));
    }
    let mut header = [0i32; HEADER_ENTRIES];
    for entry in &mut header {
        *entry = reader.i32()?;
    }
    let tag_count = header[TAG_COUNT_ENTRY];
    if tag_count <= 0 {
        return Err(format_err!("invalid SPB tag count: {tag_count}"));
    }

    let mut tags: Vec<Tag> = Vec::with_capacity(tag_count as usize);
    for _ in 0..tag_count - 1 {
        let id = reader.guid()?;
        let size = reader.i32()?;
        let key = guid::to_lower(&id);
        let def = bank
            .get(&key)
            .ok_or_else(|| format_err!("unbound property GUID: {{{key}}}"))?;
        tags.push(Tag { def, size });
    }

    let mut document = Node::new("#document");
    parse_element(&mut reader, &tags, None, &mut document, text)?;
    if document.children.len() != 1 {
        return Err(format_err!(
            "expected exactly one root element, found {}",
            document.children.len()
        ));
    }
    Ok(document.children.pop().unwrap())
}

fn parse_element(
    reader: &mut Reader,
    tags: &[Tag],
    current: Option<&str>,
    parent: &mut Node,
    text: &TextTable,
) -> Result<()> {
    let index = reader.i32()? - 1;
    if index == -1 {
        return Ok(());
    }
    let tag = usize::try_from(index)
        .ok()
        .and_then(|i| tags.get(i))
        .ok_or_else(|| format_err!("invalid tag index {index}"))?;

    match tag.def.kind {
        Kind::Set => parse_set(reader, tags, current, tag.def, parent, text),
        Kind::Property => parse_property(reader, current, tag.def, tag.size, parent, text),
        Kind::Type => Err(format_err!("unexpected type tag '{}'", tag.def.name)),
    }
}

fn parse_set(
    reader: &mut Reader,
    tags: &[Tag],
    current: Option<&str>,
    set: &Def,
    parent: &mut Node,
    text: &TextTable,
) -> Result<()> {
    let byte_length = reader.i32()?;
    if byte_length < 0 {
        return Err(format_err!("negative set length for '{}'", set.name));
    }
    let end = reader.position() + byte_length as usize;

    let needs_prefix = current != Some(set.symbol.as_str());
    let name = if needs_prefix {
        format!("{}.{}", set.symbol, set.name)
    } else {
        set.name.clone()
    };
    let mut node = Node::new(name);

    let child_symbol = Some(set.symbol.as_str());
    while reader.position() < end {
        parse_element(reader, tags, child_symbol, &mut node, text)?;
    }
    if reader.position() != end {
        return Err(format_err!(
            "set '{}' overran its declared length",
            set.name
        ));
    }

    parent.children.push(node);
    Ok(())
}

fn parse_property(
    reader: &mut Reader,
    current: Option<&str>,
    prop: &Def,
    size: i32,
    parent: &mut Node,
    text: &TextTable,
) -> Result<()> {
    let value = match prop.value_type.as_str() {
        "TEXT" | "MLTEXT" => {
            let len = read_len(reader)?;
            if len > 0 {
                text.decode(reader.bytes(len)?)?
            } else {
                String::new()
            }
        }
        "BEZIERCURVE" => {
            let len = read_len(reader)?;
            if len > 0 {
                String::from_utf8_lossy(reader.bytes(len)?)
                    .trim_end_matches('\0')
                    .to_string()
            } else {
                String::new()
            }
        }
        "FLOAT" => float_family(reader, 1, size)?,
        "FLOAT2" => float_family(reader, 2, size)?,
        "FLOAT3" => float_family(reader, 3, size)?,
        "FLOAT4" => float_family(reader, 4, size)?,
        "ULONG" => reader.u32()?.to_string(),
        "LONG" => reader.i32()?.to_string(),
        "LONG2" => format!("{},{}", reader.i32()?, reader.i32()?),
        "LONG4" => format!(
            "{},{},{},{}",
            reader.i32()?,
            reader.i32()?,
            reader.i32()?,
            reader.i32()?
        ),
        "BOOL" => bool_text(reader.i32()?),
        "DOUBLE" => dec3(reader.f64()?),
        "BYTE4" => format!(
            "{},{},{},{}",
            reader.u8()?,
            reader.u8()?,
            reader.u8()?,
            reader.u8()?
        ),
        "GUID" | "OUTPUTVALUE" => guid::to_braced_upper(&reader.guid()?),
        "INPUTBOOL" => format!("{},{}", guid_text(reader)?, bool_text(reader.i32()?)),
        "INPUTLONG" => format!("{},{}", guid_text(reader)?, reader.i32()?),
        "INPUTULONG" => format!("{},{}", guid_text(reader)?, reader.u32()?),
        "INPUTVARIANT" | "INPUTFLOAT" => {
            format!("{},{}", guid_text(reader)?, dec3_f32(reader.f32()?))
        }
        "INPUTFLOAT2" => format!(
            "{},{},{}",
            guid_text(reader)?,
            dec3_f32(reader.f32()?),
            dec3_f32(reader.f32()?)
        ),
        "INPUTFLOAT3" => format!(
            "{},{},{},{}",
            guid_text(reader)?,
            dec3_f32(reader.f32()?),
            dec3_f32(reader.f32()?),
            dec3_f32(reader.f32()?)
        ),
        "INPUTCOLOR" => format!(
            "{},{},{},{},{}",
            guid_text(reader)?,
            reader.u8()?,
            reader.u8()?,
            reader.u8()?,
            reader.u8()?
        ),
        "PBH" | "PBH32" => {
            let pitch = reader.u32()? as f64 / REVOLUTION * 360.0;
            let bank = reader.u32()? as f64 / REVOLUTION * 360.0;
            let heading = reader.u32()? as f64 / REVOLUTION * 360.0;
            reader.i32()?;
            format!("{},{},{}", dec3(pitch), dec3(bank), dec3(heading))
        }
        "ENUM" => {
            let index = reader.i32()?;
            usize::try_from(index)
                .ok()
                .and_then(|i| prop.enum_values.get(i))
                .ok_or_else(|| format_err!("enum index {index} out of range for '{}'", prop.name))?
                .clone()
        }
        "LLA" => {
            let lat = reader.i64()?;
            let lon = reader.i64()?;
            let alt_frac = reader.u32()?;
            let alt_whole = reader.i32()?;
            lla(lat, lon, alt_frac, alt_whole)
        }
        // The reference tool consumes an eight byte timestamp and emits nothing,
        // so this property contributes no element.
        "FILETIME" => {
            reader.bytes(8)?;
            return Ok(());
        }
        other => {
            return Err(format_err!(
                "unsupported value type '{other}' (size {size})"
            ))
        }
    };

    add_property(current, prop, value, parent);
    Ok(())
}

/// Read a float vector property. The declared type gives the component count
/// used for the plain form; a larger value size means the simulator stored an
/// input pin (a source GUID followed by the floats), and the exact float count
/// is derived from the size.
fn float_family(reader: &mut Reader, components: usize, size: i32) -> Result<String> {
    let plain = (components * 4) as i32;
    if size > plain && size >= GUID_SIZE + 4 && (size - GUID_SIZE) % 4 == 0 {
        let count = ((size - GUID_SIZE) / 4) as usize;
        let mut parts = Vec::with_capacity(count + 1);
        parts.push(guid::to_braced_upper(&reader.guid()?));
        for _ in 0..count {
            parts.push(dec3_f32(reader.f32()?));
        }
        Ok(parts.join(","))
    } else {
        let mut parts = Vec::with_capacity(components);
        for _ in 0..components {
            parts.push(dec3_f32(reader.f32()?));
        }
        Ok(parts.join(","))
    }
}

fn add_property(current: Option<&str>, prop: &Def, value: String, parent: &mut Node) {
    if prop.is_attribute {
        parent.attributes.push((prop.name.clone(), value));
        return;
    }
    let needs_prefix = !prop.symbol.is_empty() && current != Some(prop.symbol.as_str());
    let leaf = prop.name.trim_end();
    let name = if needs_prefix {
        format!("{}.{}", prop.symbol, leaf)
    } else {
        leaf.to_string()
    };
    let mut child = Node::new(name);
    child.text = Some(value);
    parent.children.push(child);
}

fn guid_text(reader: &mut Reader) -> Result<String> {
    Ok(guid::to_braced_upper(&reader.guid()?))
}

fn read_len(reader: &mut Reader) -> Result<usize> {
    let len = reader.i32()?;
    usize::try_from(len).map_err(|_| format_err!("negative length {len}"))
}

fn bool_text(value: i32) -> String {
    if value == 1 { "true" } else { "false" }.to_string()
}
