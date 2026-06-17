//! End to end test driven by a synthetic SPB stream and a matching set of
//! property definitions. Keeping the inputs synthetic avoids shipping any
//! simulator data while still exercising the full pipeline: propdef loading,
//! GUID resolution, the aux driven float pin logic, attributes, set nesting
//! and rendering.

use std::fs;

use sbp2xml24::{convert, Bank, Encoding, TextTable};

const PROPDEFS: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<SymbolDef name="Demo" id="{000000AA-0000-0000-0000-000000000000}">
  <PropertyDefs>
    <PropertyDef name="Ref"   id="{00000002-0000-0000-0000-000000000000}" type="GUID" xml_io="attribute"/>
    <PropertyDef name="Scale" id="{00000003-0000-0000-0000-000000000000}" type="FLOAT"/>
    <PropertyDef name="Rate"  id="{00000004-0000-0000-0000-000000000000}" type="FLOAT"/>
    <PropertyDef name="Flag"  id="{00000005-0000-0000-0000-000000000000}" type="BOOL"/>
  </PropertyDefs>
  <SetDefs>
    <SetDef name="Root" id="{00000001-0000-0000-0000-000000000000}"/>
  </SetDefs>
</SymbolDef>
"#;

const EXPECTED: &str = "\
<?xml version=\"1.0\" encoding=\"utf-8\"?>
<Demo.Root Ref=\"{11223344-5566-7788-99AA-BBCCDDEEFF00}\">
  <Scale>1.500</Scale>
  <Rate>{00000000-0000-0000-0000-000000000000},2.500</Rate>
  <Flag>true</Flag>
</Demo.Root>
";

/// Parse a canonical GUID string into the 16 byte .NET little endian layout.
fn guid_le(canonical: &str) -> [u8; 16] {
    let hex: String = canonical.chars().filter(|c| *c != '-').collect();
    let b: Vec<u8> = (0..16)
        .map(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap())
        .collect();
    [
        b[3], b[2], b[1], b[0], b[5], b[4], b[7], b[6], b[8], b[9], b[10], b[11], b[12], b[13],
        b[14], b[15],
    ]
}

/// Minimal builder for little endian SPB byte streams.
#[derive(Default)]
struct Spb {
    bytes: Vec<u8>,
}

impl Spb {
    fn u16(&mut self, v: u16) -> &mut Self {
        self.bytes.extend_from_slice(&v.to_le_bytes());
        self
    }
    fn i32(&mut self, v: i32) -> &mut Self {
        self.bytes.extend_from_slice(&v.to_le_bytes());
        self
    }
    fn f32(&mut self, v: f32) -> &mut Self {
        self.bytes.extend_from_slice(&v.to_le_bytes());
        self
    }
    fn guid(&mut self, canonical: &str) -> &mut Self {
        self.bytes.extend_from_slice(&guid_le(canonical));
        self
    }
}

#[test]
fn converts_synthetic_document() {
    // Tag table: index -> (definition GUID, value size). Body tags reference
    // these by index + 1.
    let mut spb = Spb::default();
    spb.u16(60332); // file id
    for i in 0..12 {
        spb.i32(if i == 6 { 6 } else { 0 }); // header; entry 6 is the tag count
    }
    // Five tag records (tag_count - 1).
    spb.guid("00000001-0000-0000-0000-000000000000").i32(-1); // 0: Root set
    spb.guid("00000002-0000-0000-0000-000000000000").i32(16); // 1: Ref GUID attribute
    spb.guid("00000003-0000-0000-0000-000000000000").i32(4); //  2: Scale plain float
    spb.guid("00000004-0000-0000-0000-000000000000").i32(20); // 3: Rate input float
    spb.guid("00000005-0000-0000-0000-000000000000").i32(4); //  4: Flag

    // Body: Root set containing one attribute and three child properties.
    let mut body = Spb::default();
    body.i32(2).guid("11223344-5566-7788-99AA-BBCCDDEEFF00"); // Ref attribute
    body.i32(3).f32(1.5); // Scale
    body.i32(4)
        .guid("00000000-0000-0000-0000-000000000000")
        .f32(2.5); // Rate input pin
    body.i32(5).i32(1); // Flag = true
    let body = body.bytes;

    spb.i32(1).i32(body.len() as i32); // Root set tag + byte length
    spb.bytes.extend_from_slice(&body);

    let dir = std::env::temp_dir().join("sbp2xml24_it_propdefs");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("demo.xml"), PROPDEFS).unwrap();

    let bank = Bank::load(&dir).unwrap();
    let text = TextTable::embedded();
    let xml = convert(&spb.bytes, &bank, &text, Encoding::Utf8).unwrap();

    assert_eq!(String::from_utf8(xml).unwrap(), EXPECTED);
}
