//! GUID formatting that matches the .NET `Guid(byte[16])` byte order.
//!
//! The first three fields are stored little endian, the remaining eight bytes
//! in order. This mirrors how the simulator serialises GUIDs so the rendered
//! text matches the original authored XML.

/// Format a 16 byte GUID as a lowercase, hyphenated string without braces.
pub fn to_lower(bytes: &[u8; 16]) -> String {
    let d1 = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    let d2 = u16::from_le_bytes([bytes[4], bytes[5]]);
    let d3 = u16::from_le_bytes([bytes[6], bytes[7]]);
    format!(
        "{:08x}-{:04x}-{:04x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        d1,
        d2,
        d3,
        bytes[8],
        bytes[9],
        bytes[10],
        bytes[11],
        bytes[12],
        bytes[13],
        bytes[14],
        bytes[15],
    )
}

/// Format a GUID the way the simulator writes it in XML: braces, uppercase.
pub fn to_braced_upper(bytes: &[u8; 16]) -> String {
    format!("{{{}}}", to_lower(bytes).to_uppercase())
}
