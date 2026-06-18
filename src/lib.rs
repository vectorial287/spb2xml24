//! Decompile Microsoft Flight Simulator 2024 compiled SPB property files back
//! to XML.
//!
//! The high level entry point is [`convert`], which turns an SPB byte slice
//! into a rendered XML document. It needs two inputs:
//!
//! * a [`Bank`] of property definitions, loaded from the simulator's propdefs
//!   directory with [`Bank::load`];
//! * a [`TextTable`] for the embedded text encoding, obtained from
//!   [`TextTable::embedded`].
//!
//! ```no_run
//! use std::path::Path;
//! use spb2xml24::{convert, Bank, Encoding, TextTable};
//!
//! let bank = Bank::load(Path::new("Propdefs/1.0/Common"))?;
//! let text = TextTable::embedded();
//! let spb = std::fs::read("effect.spb")?;
//! let xml = convert(&spb, &bank, &text, Encoding::Utf8)?;
//! std::fs::write("effect.xml", xml)?;
//! # Ok::<(), spb2xml24::Error>(())
//! ```

mod format;
mod guid;
mod reader;
mod spb;

pub mod error;
pub mod locate;
pub mod propdefs;
pub mod textdecode;
pub mod xml;

pub use error::{Error, Result};
pub use propdefs::Bank;
pub use textdecode::TextTable;
pub use xml::{Encoding, Node};

/// Decode an SPB byte stream and render it as an XML document.
pub fn convert(spb: &[u8], bank: &Bank, text: &TextTable, encoding: Encoding) -> Result<Vec<u8>> {
    let root = spb::parse(spb, bank, text)?;
    Ok(xml::render(&root, encoding))
}
