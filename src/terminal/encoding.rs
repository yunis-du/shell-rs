//! The character encoding of the interactive terminal byte stream.

mod iso2022_cn;
mod unicode;

use encoding_rs::{CoderResult, Decoder, EncoderResult, Encoding};
use serde::{Deserialize, Serialize};

use unicode::{Utf7Decoder, WideDecoder, WideFormat, encode_utf7};

/// Encodings offered by terminal settings. The emulator itself consumes UTF-8.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerminalEncoding {
    Utf16,
    Utf16Be,
    Utf16Le,
    Utf32,
    Utf32Be,
    Utf32Le,
    Utf7,
    #[default]
    Utf8,
    Gb18030,
    Gbk,
    Iso2022Cn,
    Big5,
    ShiftJis,
}

impl TerminalEncoding {
    fn legacy(self) -> Option<&'static Encoding> {
        match self {
            Self::Gb18030 => Some(encoding_rs::GB18030),
            Self::Gbk => Some(encoding_rs::GBK),
            Self::Big5 => Some(encoding_rs::BIG5),
            Self::ShiftJis => Some(encoding_rs::SHIFT_JIS),
            _ => None,
        }
    }

    /// `None` byte order means read a BOM or assume big-endian. Writes in
    /// that mode are big-endian without a BOM; the two streams are separate.
    fn wide(self) -> Option<(WideFormat, Option<bool>)> {
        Some(match self {
            Self::Utf16 => (WideFormat::Utf16, None),
            Self::Utf16Be => (WideFormat::Utf16, Some(false)),
            Self::Utf16Le => (WideFormat::Utf16, Some(true)),
            Self::Utf32 => (WideFormat::Utf32, None),
            Self::Utf32Be => (WideFormat::Utf32, Some(false)),
            Self::Utf32Le => (WideFormat::Utf32, Some(true)),
            _ => return None,
        })
    }

    pub(super) fn uses_wide_units(self) -> bool {
        self.wide().is_some()
    }

    pub(super) fn macos_locale(self) -> Option<&'static str> {
        Some(match self {
            Self::Utf8 => "UTF-8",
            Self::Gb18030 => "zh_CN.GB18030",
            Self::Gbk => "zh_CN.GBK",
            Self::Big5 => "zh_TW.Big5",
            Self::ShiftJis => "ja_JP.SJIS",
            _ => return None,
        })
    }

    /// Terminal replies and mouse reports are already protocol bytes. Only
    /// the wide formats change how those bytes are represented on the wire.
    pub(super) fn encode_protocol(self, bytes: Vec<u8>) -> Vec<u8> {
        if let Some((format, little)) = self.wide() {
            format.encode_protocol(&bytes, little.unwrap_or(false))
        } else {
            bytes
        }
    }

    /// Reject unmappable text instead of typing a different command.
    pub(super) fn encode_input(self, bytes: Vec<u8>) -> Option<Vec<u8>> {
        if self == Self::Utf8 {
            return Some(bytes);
        }
        let text = std::str::from_utf8(&bytes).ok()?;
        if let Some((format, little)) = self.wide() {
            return Some(format.encode(text, little.unwrap_or(false)));
        }
        if self == Self::Utf7 {
            return Some(encode_utf7(text));
        }
        if bytes.is_ascii() {
            return Some(bytes);
        }
        if self == Self::Iso2022Cn {
            return iso2022_cn::encode(text);
        }
        let mut encoder = self.legacy()?.new_encoder();
        let mut encoded = Vec::with_capacity(
            encoder.max_buffer_length_from_utf8_without_replacement(text.len())?,
        );
        let (result, _) =
            encoder.encode_from_utf8_to_vec_without_replacement(text, &mut encoded, true);
        (result == EncoderResult::InputEmpty).then_some(encoded)
    }
}

/// Keeps partial characters and shift state across transport reads.
/// UTF-8 already streams through the terminal parser unchanged.
pub(super) struct TerminalDecoder {
    state: DecoderState,
    decoded: String,
}

enum DecoderState {
    Utf8,
    Legacy(Decoder),
    Wide(WideDecoder),
    Utf7(Utf7Decoder),
    Iso2022Cn(iso2022_cn::Iso2022CnDecoder),
}

impl TerminalDecoder {
    pub(super) fn new(encoding: TerminalEncoding) -> Self {
        let state = if let Some((format, little)) = encoding.wide() {
            DecoderState::Wide(WideDecoder::new(format, little))
        } else {
            match encoding {
                TerminalEncoding::Utf7 => DecoderState::Utf7(Utf7Decoder::default()),
                TerminalEncoding::Iso2022Cn => DecoderState::Iso2022Cn(Default::default()),
                _ => encoding.legacy().map_or(DecoderState::Utf8, |legacy| {
                    DecoderState::Legacy(legacy.new_decoder_without_bom_handling())
                }),
            }
        };
        Self {
            state,
            decoded: String::new(),
        }
    }

    pub(super) fn decode<'a>(&'a mut self, bytes: &'a [u8], last: bool) -> &'a [u8] {
        let out = &mut self.decoded;
        out.clear();
        match &mut self.state {
            DecoderState::Utf8 => return bytes,
            DecoderState::Wide(decoder) => decoder.decode(bytes, out, last),
            DecoderState::Utf7(decoder) => decoder.decode(bytes, out, last),
            DecoderState::Iso2022Cn(decoder) => decoder.decode(bytes, out, last),
            DecoderState::Legacy(decoder) => {
                out.reserve(
                    decoder
                        .max_utf8_buffer_length(bytes.len())
                        .expect("terminal output size"),
                );
                let (result, read, _) = decoder.decode_to_string(bytes, out, last);
                debug_assert_eq!((result, read), (CoderResult::InputEmpty, bytes.len()));
                if last {
                    *decoder = decoder.encoding().new_decoder_without_bom_handling();
                }
            }
        }
        out.as_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode(encoding: TerminalEncoding, wire: &[u8]) -> String {
        let mut decoder = TerminalDecoder::new(encoding);
        let mut text = Vec::new();
        // Deliver even BOMs, surrogate pairs and escape sequences one byte at a time.
        for byte in wire {
            text.extend(decoder.decode(std::slice::from_ref(byte), false));
        }
        text.extend(decoder.decode(&[], true));
        String::from_utf8(text).unwrap()
    }

    #[test]
    fn wide_and_utf7_wire_encodings() {
        let text = "中文😀+";
        let utf16_be = b"\x4e\x2d\x65\x87\xd8\x3d\xde\x00\x00\x2b";
        let utf16_le = b"\x2d\x4e\x87\x65\x3d\xd8\x00\xde\x2b\x00";
        let utf32_be = b"\x00\x00\x4e\x2d\x00\x00\x65\x87\x00\x01\xf6\x00\x00\x00\x00\x2b";
        let utf32_le = b"\x2d\x4e\x00\x00\x87\x65\x00\x00\x00\xf6\x01\x00\x2b\x00\x00\x00";
        for (encoding, wire) in [
            (TerminalEncoding::Utf16, utf16_be.as_slice()),
            (TerminalEncoding::Utf16Be, utf16_be.as_slice()),
            (TerminalEncoding::Utf16Le, utf16_le.as_slice()),
            (TerminalEncoding::Utf32, utf32_be.as_slice()),
            (TerminalEncoding::Utf32Be, utf32_be.as_slice()),
            (TerminalEncoding::Utf32Le, utf32_le.as_slice()),
            (TerminalEncoding::Utf7, b"+Ti1lh9g93gA-+-"),
            (TerminalEncoding::Utf8, text.as_bytes()),
        ] {
            assert_eq!(decode(encoding, wire), text, "{encoding:?}");
            assert_eq!(
                encoding.encode_input(text.as_bytes().to_vec()).as_deref(),
                Some(wire),
                "{encoding:?}"
            );
        }
        assert_eq!(
            decode(
                TerminalEncoding::Utf16,
                &[b"\xff\xfe", utf16_le.as_slice()].concat()
            ),
            text
        );
        assert_eq!(
            decode(
                TerminalEncoding::Utf32,
                &[b"\xff\xfe\0\0", utf32_le.as_slice()].concat()
            ),
            text
        );
        assert_eq!(decode(TerminalEncoding::Utf16Le, b"\x3d\xd8\x00"), "��");
        assert_eq!(decode(TerminalEncoding::Utf32Be, b"\x00\x11\x00\x00"), "�");
    }

    #[test]
    fn iso2022_cn_uses_all_three_sets_and_preserves_terminal_escapes() {
        let encoding = TerminalEncoding::Iso2022Cn;
        let text = "中文體乂";
        // GB 2312, CNS plane 1, then a single shift to CNS plane 2.
        let wire = b"\x1b$)A\x0eVPND\x1b$)G|U\x1b$*H\x1bN!!\x0f";
        assert_eq!(
            encoding.encode_input(text.as_bytes().to_vec()).as_deref(),
            Some(wire.as_slice())
        );
        assert_eq!(decode(encoding, wire), text);
        assert_eq!(
            decode(encoding, b"\x1b$)A\x0eVP\x1b[31mND\x1b[0m\x0f!"),
            "中\x1b[31m文\x1b[0m!"
        );
    }

    #[test]
    fn gbk_does_not_emit_gb18030_four_byte_sequences() {
        assert_eq!(decode(TerminalEncoding::Gbk, b"\xd6\xd0\xce\xc4"), "中文");
        let text = "😀".as_bytes().to_vec();
        assert!(TerminalEncoding::Gbk.encode_input(text.clone()).is_none());
        assert_eq!(
            TerminalEncoding::Gb18030.encode_input(text).unwrap().len(),
            4
        );
    }
}
