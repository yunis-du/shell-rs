use base64::{Engine as _, engine::general_purpose::STANDARD_NO_PAD};

#[derive(Clone, Copy)]
pub(super) enum WideFormat {
    Utf16,
    Utf32,
}

impl WideFormat {
    fn width(self) -> usize {
        match self {
            Self::Utf16 => 2,
            Self::Utf32 => 4,
        }
    }

    pub(super) fn encode(self, text: &str, little: bool) -> Vec<u8> {
        match self {
            Self::Utf16 => text
                .encode_utf16()
                .flat_map(|unit| {
                    if little {
                        unit.to_le_bytes()
                    } else {
                        unit.to_be_bytes()
                    }
                })
                .collect(),
            Self::Utf32 => text
                .chars()
                .flat_map(|ch| {
                    if little {
                        (ch as u32).to_le_bytes()
                    } else {
                        (ch as u32).to_be_bytes()
                    }
                })
                .collect(),
        }
    }

    /// Some VT mouse protocols use an octet above ASCII as a coordinate.
    /// Widen each protocol octet without treating it as UTF-8 text.
    pub(super) fn encode_protocol(self, bytes: &[u8], little: bool) -> Vec<u8> {
        let mut wire = Vec::with_capacity(bytes.len() * self.width());
        for &byte in bytes {
            let unit = if little {
                u32::from(byte).to_le_bytes()
            } else {
                u32::from(byte).to_be_bytes()
            };
            let from = if little { 0 } else { unit.len() - self.width() };
            wire.extend_from_slice(&unit[from..from + self.width()]);
        }
        wire
    }
}

pub(super) struct WideDecoder {
    format: WideFormat,
    little: bool,
    detect_bom: bool,
    at_start: bool,
    pending: [u8; 4],
    used: usize,
    utf16: Utf16,
}

impl WideDecoder {
    pub(super) fn new(format: WideFormat, little: Option<bool>) -> Self {
        Self {
            format,
            little: little.unwrap_or(false),
            detect_bom: little.is_none(),
            at_start: true,
            pending: [0; 4],
            used: 0,
            utf16: Utf16::default(),
        }
    }

    pub(super) fn decode(&mut self, bytes: &[u8], out: &mut String, last: bool) {
        for &byte in bytes {
            self.pending[self.used] = byte;
            self.used += 1;
            if self.used == self.format.width() {
                self.used = 0;
                self.unit(out);
            }
        }
        if last {
            self.utf16.finish(out);
            if self.used != 0 {
                out.push(char::REPLACEMENT_CHARACTER);
                self.used = 0;
            }
        }
    }

    fn unit(&mut self, out: &mut String) {
        let bytes = self.pending;
        if std::mem::take(&mut self.at_start) {
            let bom =
                match (self.format, bytes) {
                    (WideFormat::Utf16, [0xfe, 0xff, ..])
                    | (WideFormat::Utf32, [0, 0, 0xfe, 0xff]) => Some(false),
                    (WideFormat::Utf16, [0xff, 0xfe, ..])
                    | (WideFormat::Utf32, [0xff, 0xfe, 0, 0]) => Some(true),
                    _ => None,
                };
            if let Some(little) = bom {
                if self.detect_bom {
                    self.little = little;
                }
                if little == self.little {
                    return;
                }
            }
        }
        match self.format {
            WideFormat::Utf16 => {
                let pair = [bytes[0], bytes[1]];
                self.utf16.push(
                    if self.little {
                        u16::from_le_bytes(pair)
                    } else {
                        u16::from_be_bytes(pair)
                    },
                    out,
                );
            }
            WideFormat::Utf32 => {
                let value = if self.little {
                    u32::from_le_bytes(bytes)
                } else {
                    u32::from_be_bytes(bytes)
                };
                out.push(char::from_u32(value).unwrap_or(char::REPLACEMENT_CHARACTER));
            }
        }
    }
}

#[derive(Default)]
struct Utf16(Option<u16>);

impl Utf16 {
    fn push(&mut self, unit: u16, out: &mut String) {
        if let Some(high) = self.0.take() {
            if (0xdc00..=0xdfff).contains(&unit) {
                let value =
                    0x1_0000 + ((u32::from(high) - 0xd800) << 10) + (u32::from(unit) - 0xdc00);
                out.push(char::from_u32(value).expect("UTF-16 surrogate pair"));
                return;
            }
            out.push(char::REPLACEMENT_CHARACTER);
        }
        if (0xd800..=0xdbff).contains(&unit) {
            self.0 = Some(unit);
        } else {
            out.push(char::from_u32(u32::from(unit)).unwrap_or(char::REPLACEMENT_CHARACTER));
        }
    }

    fn finish(&mut self, out: &mut String) {
        if self.0.take().is_some() {
            out.push(char::REPLACEMENT_CHARACTER);
        }
    }
}

/// UTF-7 (RFC 2152). Leave ASCII VT control bytes direct on the wire.
pub(super) fn encode_utf7(text: &str) -> Vec<u8> {
    let mut out = String::with_capacity(text.len());
    let mut wide = Vec::new();
    let flush = |wide: &mut Vec<u8>, out: &mut String| {
        if !wide.is_empty() {
            out.push('+');
            STANDARD_NO_PAD.encode_string(&*wide, out);
            out.push('-');
            wide.clear();
        }
    };
    for ch in text.chars() {
        if ch.is_ascii() && !matches!(ch, '\\' | '~' | '\x7f') {
            flush(&mut wide, &mut out);
            if ch == '+' {
                out.push_str("+-");
            } else {
                out.push(ch);
            }
        } else {
            for unit in ch.encode_utf16(&mut [0; 2]) {
                wide.extend(unit.to_be_bytes());
            }
        }
    }
    flush(&mut wide, &mut out);
    out.into_bytes()
}

#[derive(Default)]
pub(super) struct Utf7Decoder {
    shifted: bool,
    saw_digit: bool,
    bits: u32,
    used: u8,
    utf16: Utf16,
}

impl Utf7Decoder {
    pub(super) fn decode(&mut self, bytes: &[u8], out: &mut String, last: bool) {
        for &byte in bytes {
            if self.shifted {
                if let Some(digit) = base64_digit(byte) {
                    self.saw_digit = true;
                    self.bits = (self.bits << 6) | u32::from(digit);
                    self.used += 6;
                    if self.used >= 16 {
                        self.used -= 16;
                        self.utf16.push((self.bits >> self.used) as u16, out);
                        self.bits &= (1 << self.used) - 1;
                    }
                    continue;
                }
                if byte == b'-' && !self.saw_digit {
                    out.push('+');
                    self.shifted = false;
                    continue;
                }
                self.end_shift(out);
                if byte == b'-' {
                    continue;
                }
            }
            if byte == b'+' {
                self.shifted = true;
            } else {
                out.push(if byte.is_ascii() {
                    char::from(byte)
                } else {
                    char::REPLACEMENT_CHARACTER
                });
            }
        }
        if last && self.shifted {
            self.end_shift(out);
        }
    }

    fn end_shift(&mut self, out: &mut String) {
        self.utf16.finish(out);
        if !self.saw_digit || self.used >= 6 || self.bits != 0 {
            out.push(char::REPLACEMENT_CHARACTER);
        }
        *self = Self::default();
    }
}

fn base64_digit(byte: u8) -> Option<u8> {
    Some(match byte {
        b'A'..=b'Z' => byte - b'A',
        b'a'..=b'z' => byte - b'a' + 26,
        b'0'..=b'9' => byte - b'0' + 52,
        b'+' => 62,
        b'/' => 63,
        _ => return None,
    })
}
