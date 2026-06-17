//! Decoder for the simulator's position dependent text encoding.
//!
//! Compiled `TEXT` and `MLTEXT` values are stored with a substitution table
//! where the cipher byte depends on both the source character and its position
//! in the string (the position wraps every [`PERIOD`] characters). The lookup
//! table is embedded from `assets/textdecode.bin`; see `tools/extract_textdecode.py`
//! for how that asset is produced.

use crate::error::{format_err, Result};

const PERIOD: usize = 250;
const CODES: usize = 256;

/// Raw table asset: 256 records, each a little endian `u16` length followed by
/// that many cipher bytes for the matching source character code.
static TABLE: &[u8] = include_bytes!("../assets/textdecode.bin");

/// Reverse lookup mapping `(cipher byte, position)` back to a character code.
/// `-1` marks an unused cell.
pub struct TextTable {
    map: Vec<i16>,
}

impl TextTable {
    /// Build the decoder from the embedded table. Panics only if the embedded
    /// asset is malformed, which would be a build time error rather than a
    /// runtime one.
    pub fn embedded() -> Self {
        let mut map = vec![-1i16; CODES * PERIOD];
        let mut offset = 0usize;
        for code in 0..CODES {
            let len = u16::from_le_bytes([TABLE[offset], TABLE[offset + 1]]) as usize;
            offset += 2;
            let row = &TABLE[offset..offset + len];
            offset += len;
            for (pos, &cipher) in row.iter().enumerate() {
                map[cipher as usize * PERIOD + pos] = code as i16;
            }
        }
        TextTable { map }
    }

    /// Decode a cipher byte slice. The final byte is a terminator that must map
    /// back to the null character.
    pub fn decode(&self, encoded: &[u8]) -> Result<String> {
        if encoded.is_empty() {
            return Ok(String::new());
        }

        let mut out = String::with_capacity(encoded.len());
        for (i, &byte) in encoded[..encoded.len() - 1].iter().enumerate() {
            let code = self.map[byte as usize * PERIOD + (i % PERIOD)];
            if code < 0 {
                return Err(format_err!("text decode failed at byte {i}"));
            }
            out.push(char::from_u32(code as u32).unwrap());
        }

        let last = encoded.len() - 1;
        if self.map[encoded[last] as usize * PERIOD + (last % PERIOD)] != 0 {
            return Err(format_err!("text decode terminator mismatch"));
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Rebuild the forward cipher from the reverse table so the round trip can
    /// be exercised without shipping a second copy of the data.
    fn encode(table: &TextTable, text: &str) -> Vec<u8> {
        let cipher_for = |code: u32, pos: usize| -> u8 {
            (0..CODES)
                .find(|&b| table.map[b * PERIOD + pos] == code as i16)
                .expect("every code/position has a cipher byte") as u8
        };
        let mut out: Vec<u8> = text
            .chars()
            .enumerate()
            .map(|(i, c)| cipher_for(c as u32, i % PERIOD))
            .collect();
        out.push(cipher_for(0, text.len() % PERIOD));
        out
    }

    #[test]
    fn round_trips_ascii() {
        let table = TextTable::embedded();
        for sample in ["", "A", "Hello, world!", "Effect_Name 123"] {
            let encoded = encode(&table, sample);
            assert_eq!(table.decode(&encoded).unwrap(), sample);
        }
    }
}
