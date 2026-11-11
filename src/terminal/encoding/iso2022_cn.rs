//! RFC 1922: GB 2312 and CNS 11643 planes 1 and 2. The mapping is generated
//! by scripts/generate-iso2022-cn-table.py, so every target uses the same data.

use std::collections::HashMap;
use std::sync::OnceLock;

const CELLS: usize = 94 * 94;
const TABLE: &[u8; 3 * CELLS * 2] = include_bytes!("iso2022-cn.bin");
const GB: &[u8] = b"\x1b$)A";
const CNS1: &[u8] = b"\x1b$)G";
const CNS2: &[u8] = b"\x1b$*H";
const SS2: &[u8] = b"\x1bN";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Set {
    Gb,
    Cns1,
    Cns2,
}

impl Set {
    const ALL: [Self; 3] = [Self::Gb, Self::Cns1, Self::Cns2];

    fn designation(self) -> &'static [u8] {
        match self {
            Self::Gb => GB,
            Self::Cns1 => CNS1,
            Self::Cns2 => CNS2,
        }
    }

    fn character(self, cell: usize) -> Option<char> {
        let at = (self as usize * CELLS + cell) * 2;
        let value = u16::from_le_bytes([TABLE[at], TABLE[at + 1]]);
        (value != 0)
            .then(|| char::from_u32(u32::from(value)))
            .flatten()
    }
}

pub(super) fn encode(text: &str) -> Option<Vec<u8>> {
    static CODES: OnceLock<HashMap<char, (Set, [u8; 2])>> = OnceLock::new();
    let mut out = Vec::with_capacity(text.len());
    let mut g1 = None;
    let mut g2 = false;
    let mut shifted = false;
    for ch in text.chars() {
        if ch.is_ascii() {
            if std::mem::take(&mut shifted) {
                out.push(0x0f);
            }
            out.push(ch as u8);
            if ch == '\n' {
                g1 = None;
                g2 = false;
            }
            continue;
        }
        let codes = CODES.get_or_init(|| {
            let mut codes = HashMap::new();
            for set in Set::ALL {
                for cell in 0..CELLS {
                    if let Some(ch) = set.character(cell) {
                        let pair = [b'!' + (cell / 94) as u8, b'!' + (cell % 94) as u8];
                        codes.entry(ch).or_insert((set, pair));
                    }
                }
            }
            codes
        });
        let &(set, pair) = codes.get(&ch)?;
        if set == Set::Cns2 {
            if !std::mem::replace(&mut g2, true) {
                out.extend(CNS2);
            }
            out.extend(SS2);
        } else {
            if g1 != Some(set) {
                out.extend(set.designation());
                g1 = Some(set);
            }
            if !std::mem::replace(&mut shifted, true) {
                out.push(0x0e);
            }
        }
        out.extend(pair);
    }
    if shifted {
        out.push(0x0f);
    }
    Some(out)
}

#[derive(Default)]
pub(super) struct Iso2022CnDecoder {
    g1: Option<Set>,
    g2: bool,
    shifted: bool,
    single_shift: bool,
    lead: Option<u8>,
    escape: [u8; 4],
    escape_len: usize,
    control: Option<Control>,
}

impl Iso2022CnDecoder {
    pub(super) fn decode(&mut self, bytes: &[u8], out: &mut String, last: bool) {
        for &byte in bytes {
            if self.control.is_some() {
                self.terminal_control(byte, out);
            } else if self.escape_len != 0 {
                self.escape(byte, out);
            } else {
                self.character_byte(byte, out);
            }
        }
        if last {
            self.end_character(out);
            self.forward_escape(out);
            *self = Self::default();
        }
    }

    fn character_byte(&mut self, byte: u8, out: &mut String) {
        let set = if self.single_shift {
            Some(Set::Cns2)
        } else if self.shifted {
            self.g1
        } else {
            None
        };
        if let Some(set) = set
            && (b'!'..=b'~').contains(&byte)
        {
            if let Some(lead) = self.lead.take() {
                let cell = usize::from(lead - b'!') * 94 + usize::from(byte - b'!');
                out.push(set.character(cell).unwrap_or(char::REPLACEMENT_CHARACTER));
                self.single_shift = false;
            } else {
                self.lead = Some(byte);
            }
            return;
        }

        self.end_character(out);
        match byte {
            0x1b => {
                self.escape[0] = byte;
                self.escape_len = 1;
            }
            0x0e if self.g1.is_some() => self.shifted = true,
            0x0f if self.g1.is_some() => self.shifted = false,
            b'\n' => {
                self.g1 = None;
                self.g2 = false;
                self.shifted = false;
                out.push('\n');
            }
            _ => out.push(ascii(byte)),
        }
    }

    fn end_character(&mut self, out: &mut String) {
        if self.lead.take().is_some() || self.single_shift {
            out.push(char::REPLACEMENT_CHARACTER);
        }
        self.single_shift = false;
    }

    fn escape(&mut self, byte: u8, out: &mut String) {
        self.escape[self.escape_len] = byte;
        self.escape_len += 1;
        let prefix = &self.escape[..self.escape_len];
        for set in Set::ALL {
            let designation = set.designation();
            if prefix == designation {
                match set {
                    Set::Cns2 => self.g2 = true,
                    _ => self.g1 = Some(set),
                }
                self.escape_len = 0;
                return;
            }
            if designation.starts_with(prefix) {
                return;
            }
        }
        if prefix == SS2 && self.g2 {
            self.single_shift = true;
            self.escape_len = 0;
        } else {
            self.forward_escape(out);
        }
    }

    /// Preserve VT sequences such as CSI colors even in a shifted region.
    fn forward_escape(&mut self, out: &mut String) {
        if self.escape_len == 0 {
            return;
        }
        out.push('\x1b');
        self.control = Some(Control::Escape);
        for i in 1..std::mem::take(&mut self.escape_len) {
            self.terminal_control(self.escape[i], out);
        }
    }

    fn terminal_control(&mut self, byte: u8, out: &mut String) {
        out.push(ascii(byte));
        self.control = match (self.control, byte) {
            (Some(Control::String(true)), b'\\') | (Some(Control::String(_)), 0x07) => None,
            (Some(Control::String(_)), byte) => Some(Control::String(byte == 0x1b)),
            (_, 0x1b) => Some(Control::Escape),
            (Some(Control::Escape), b'[') => Some(Control::Csi),
            (Some(Control::Escape), b']' | b'P' | b'X' | b'^' | b'_') => {
                Some(Control::String(false))
            }
            (Some(Control::Escape), 0x20..=0x2f) => Some(Control::Escape),
            (Some(Control::Csi), 0x00..=0x3f) => Some(Control::Csi),
            _ => None,
        };
    }
}

#[derive(Clone, Copy)]
enum Control {
    Escape,
    Csi,
    /// An ANSI string; true after ESC, waiting for a possible ST.
    String(bool),
}

fn ascii(byte: u8) -> char {
    if byte.is_ascii() {
        char::from(byte)
    } else {
        char::REPLACEMENT_CHARACTER
    }
}
