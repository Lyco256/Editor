//! Deterministic text encoding detection/conversion and line-ending inspection.

use std::borrow::Cow;

use chardetng::EncodingDetector;
use encoding_rs::{Encoding, UTF_8};
use serde::{Deserialize, Serialize};
use thiserror::Error;

const UTF8_BOM: &[u8] = &[0xEF, 0xBB, 0xBF];
const UTF16_LE_BOM: &[u8] = &[0xFF, 0xFE];
const UTF16_BE_BOM: &[u8] = &[0xFE, 0xFF];

/// An encoding supported by Editor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EncodingKind {
    Utf8,
    Utf16Le,
    Utf16Be,
    /// Canonical `encoding_rs` name, such as `Shift_JIS` or `windows-1252`.
    Legacy(String),
}

impl EncodingKind {
    /// Resolves the aliases supplied by `encoding_rs`, plus explicit UTF-16 labels.
    ///
    /// # Errors
    ///
    /// Returns [`EncodingError::UnsupportedEncoding`] when the label is unknown.
    pub fn for_label(label: &str) -> Result<Self, EncodingError> {
        let normalized = label.trim().to_ascii_lowercase();
        match normalized.as_str() {
            "utf-16" | "utf-16le" | "utf16le" => Ok(Self::Utf16Le),
            "utf-16be" | "utf16be" => Ok(Self::Utf16Be),
            _ => {
                let encoding = Encoding::for_label(normalized.as_bytes()).ok_or_else(|| {
                    EncodingError::UnsupportedEncoding {
                        label: label.to_owned(),
                    }
                })?;
                if encoding == UTF_8 {
                    Ok(Self::Utf8)
                } else {
                    Ok(Self::Legacy(encoding.name().to_owned()))
                }
            }
        }
    }

    /// Returns a stable canonical label.
    #[must_use]
    pub fn canonical_name(&self) -> &str {
        match self {
            Self::Utf8 => "UTF-8",
            Self::Utf16Le => "UTF-16LE",
            Self::Utf16Be => "UTF-16BE",
            Self::Legacy(name) => name,
        }
    }

    fn encoding_rs(&self) -> Result<&'static Encoding, EncodingError> {
        match self {
            Self::Utf8 => Ok(UTF_8),
            Self::Legacy(name) => Encoding::for_label(name.as_bytes()).ok_or_else(|| {
                EncodingError::UnsupportedEncoding {
                    label: name.clone(),
                }
            }),
            Self::Utf16Le | Self::Utf16Be => Err(EncodingError::UnsupportedEncoding {
                label: self.canonical_name().to_owned(),
            }),
        }
    }
}

/// Behavior when input bytes or output characters cannot be represented.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodePolicy {
    Strict,
    Replace,
}

/// Detected line-ending form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LineEndings {
    None,
    Lf,
    Crlf,
    Mixed,
}

/// Decoded document plus the metadata needed to preserve it on save.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedText {
    pub text: String,
    pub encoding: EncodingKind,
    pub had_bom: bool,
    pub had_replacements: bool,
    pub line_endings: LineEndings,
}

/// Typed encoding failures.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum EncodingError {
    #[error("unsupported encoding label: {label}")]
    UnsupportedEncoding { label: String },
    #[error("input is malformed {encoding} data")]
    MalformedInput { encoding: String },
    #[error("text contains characters that cannot be represented in {encoding}")]
    UnmappableText { encoding: String },
}

/// Detects an encoding in BOM, UTF-8, confident heuristic, configured-fallback order.
///
/// # Errors
///
/// Returns [`EncodingError::UnsupportedEncoding`] if the fallback label is invalid.
pub fn detect_encoding(
    bytes: &[u8],
    fallback: &EncodingKind,
) -> Result<(EncodingKind, bool), EncodingError> {
    if bytes.starts_with(UTF8_BOM) {
        return Ok((EncodingKind::Utf8, true));
    }
    if bytes.starts_with(UTF16_LE_BOM) {
        return Ok((EncodingKind::Utf16Le, true));
    }
    if bytes.starts_with(UTF16_BE_BOM) {
        return Ok((EncodingKind::Utf16Be, true));
    }
    if std::str::from_utf8(bytes).is_ok() {
        return Ok((EncodingKind::Utf8, false));
    }

    let mut detector = EncodingDetector::new();
    detector.feed(bytes, true);
    let (guess, confident) = detector.guess_assess(None, false);
    if confident {
        return Ok((EncodingKind::Legacy(guess.name().to_owned()), false));
    }
    // Validate a dynamically constructed legacy fallback before returning it.
    if let EncodingKind::Legacy(_) = fallback {
        fallback.encoding_rs()?;
    }
    Ok((fallback.clone(), false))
}

/// Detects and decodes a document with the requested malformed-input policy.
///
/// # Errors
///
/// Returns [`EncodingError::MalformedInput`] when strict decoding encounters replacement data.
pub fn decode(
    bytes: &[u8],
    fallback: &EncodingKind,
    policy: DecodePolicy,
) -> Result<DecodedText, EncodingError> {
    let (encoding, had_bom) = detect_encoding(bytes, fallback)?;
    let payload = match &encoding {
        EncodingKind::Utf8 if had_bom => &bytes[UTF8_BOM.len()..],
        EncodingKind::Utf16Le if had_bom => &bytes[UTF16_LE_BOM.len()..],
        EncodingKind::Utf16Be if had_bom => &bytes[UTF16_BE_BOM.len()..],
        _ => bytes,
    };
    let (text, had_replacements) = decode_as(payload, &encoding)?;
    if had_replacements && policy == DecodePolicy::Strict {
        return Err(EncodingError::MalformedInput {
            encoding: encoding.canonical_name().to_owned(),
        });
    }
    let line_endings = inspect_line_endings(&text);
    Ok(DecodedText {
        text,
        encoding,
        had_bom,
        had_replacements,
        line_endings,
    })
}

fn decode_as(bytes: &[u8], encoding: &EncodingKind) -> Result<(String, bool), EncodingError> {
    match encoding {
        EncodingKind::Utf16Le => Ok(decode_utf16(bytes, true)),
        EncodingKind::Utf16Be => Ok(decode_utf16(bytes, false)),
        _ => {
            let (decoded, had_errors) = encoding.encoding_rs()?.decode_without_bom_handling(bytes);
            Ok((decoded.into_owned(), had_errors))
        }
    }
}

fn decode_utf16(bytes: &[u8], little_endian: bool) -> (String, bool) {
    let odd_byte = bytes.len() % 2 != 0;
    let units = bytes.chunks_exact(2).map(|pair| {
        if little_endian {
            u16::from_le_bytes([pair[0], pair[1]])
        } else {
            u16::from_be_bytes([pair[0], pair[1]])
        }
    });
    let mut had_errors = odd_byte;
    let text: String = char::decode_utf16(units)
        .map(|result| {
            if let Ok(ch) = result {
                ch
            } else {
                had_errors = true;
                char::REPLACEMENT_CHARACTER
            }
        })
        .collect();
    (text, had_errors)
}

/// Encodes text, optionally retaining a BOM. Strict mode rejects lossy legacy conversion.
///
/// # Errors
///
/// Returns [`EncodingError::UnmappableText`] when strict legacy conversion would lose data.
pub fn encode(
    text: &str,
    encoding: &EncodingKind,
    with_bom: bool,
    policy: DecodePolicy,
) -> Result<Vec<u8>, EncodingError> {
    match encoding {
        EncodingKind::Utf8 => {
            let mut output = Vec::with_capacity(text.len() + usize::from(with_bom) * 3);
            if with_bom {
                output.extend_from_slice(UTF8_BOM);
            }
            output.extend_from_slice(text.as_bytes());
            Ok(output)
        }
        EncodingKind::Utf16Le | EncodingKind::Utf16Be => {
            let mut output = Vec::with_capacity(text.len() * 2 + usize::from(with_bom) * 2);
            if with_bom {
                output.extend_from_slice(if *encoding == EncodingKind::Utf16Le {
                    UTF16_LE_BOM
                } else {
                    UTF16_BE_BOM
                });
            }
            for unit in text.encode_utf16() {
                let bytes = if *encoding == EncodingKind::Utf16Le {
                    unit.to_le_bytes()
                } else {
                    unit.to_be_bytes()
                };
                output.extend_from_slice(&bytes);
            }
            Ok(output)
        }
        EncodingKind::Legacy(_) => {
            let (encoded, _, had_errors) = encoding.encoding_rs()?.encode(text);
            if had_errors && policy == DecodePolicy::Strict {
                return Err(EncodingError::UnmappableText {
                    encoding: encoding.canonical_name().to_owned(),
                });
            }
            Ok(match encoded {
                Cow::Borrowed(value) => value.to_vec(),
                Cow::Owned(value) => value,
            })
        }
    }
}

/// Counts LF and CRLF sequences and surfaces mixed documents.
#[must_use]
pub fn inspect_line_endings(text: &str) -> LineEndings {
    let bytes = text.as_bytes();
    let mut lf = 0_usize;
    let mut crlf = 0_usize;
    for (index, byte) in bytes.iter().copied().enumerate() {
        if byte == b'\n' {
            if index > 0 && bytes[index - 1] == b'\r' {
                crlf += 1;
            } else {
                lf += 1;
            }
        }
    }
    match (lf, crlf) {
        (0, 0) => LineEndings::None,
        (0, _) => LineEndings::Crlf,
        (_, 0) => LineEndings::Lf,
        _ => LineEndings::Mixed,
    }
}

/// Normalizes recognized line endings while leaving other text untouched.
#[must_use]
pub fn normalize_line_endings(text: &str, target: LineEndings) -> String {
    let separator = match target {
        LineEndings::Crlf => "\r\n",
        LineEndings::Lf => "\n",
        LineEndings::None | LineEndings::Mixed => return text.to_owned(),
    };
    text.replace("\r\n", "\n").replace('\n', separator)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bom_wins_over_utf8_and_heuristic() {
        let fallback = EncodingKind::for_label("windows-1252").expect("fallback");
        assert_eq!(
            detect_encoding(b"\xEF\xBB\xBFhello", &fallback).expect("detect"),
            (EncodingKind::Utf8, true)
        );
    }

    #[test]
    fn valid_utf8_wins_over_configured_fallback() {
        let fallback = EncodingKind::for_label("Shift_JIS").expect("fallback");
        assert_eq!(
            detect_encoding("日本語".as_bytes(), &fallback).expect("detect"),
            (EncodingKind::Utf8, false)
        );
    }

    #[test]
    fn shift_jis_and_windows_code_page_round_trip() {
        for (label, text) in [("Shift_JIS", "日本語"), ("windows-1252", "café")] {
            let encoding = EncodingKind::for_label(label).expect("known encoding");
            let bytes = encode(text, &encoding, false, DecodePolicy::Strict).expect("encode");
            let decoded = decode_as(&bytes, &encoding).expect("decode");
            assert_eq!(decoded, (text.to_owned(), false));
        }
    }

    #[test]
    fn utf_bom_round_trips() {
        for encoding in [
            EncodingKind::Utf8,
            EncodingKind::Utf16Le,
            EncodingKind::Utf16Be,
        ] {
            let bytes = encode("A日本", &encoding, true, DecodePolicy::Strict).expect("encode");
            let decoded =
                decode(&bytes, &EncodingKind::Utf8, DecodePolicy::Strict).expect("decode");
            assert_eq!(decoded.text, "A日本");
            assert_eq!(decoded.encoding, encoding);
            assert!(decoded.had_bom);
        }
    }

    #[test]
    fn malformed_utf16_has_explicit_strict_and_replacement_policies() {
        let bytes = [0xFF, 0xFE, 0x00];
        assert!(matches!(
            decode(&bytes, &EncodingKind::Utf8, DecodePolicy::Strict),
            Err(EncodingError::MalformedInput { .. })
        ));
        let replaced = decode(&bytes, &EncodingKind::Utf8, DecodePolicy::Replace).expect("replace");
        assert!(replaced.had_replacements);
    }

    #[test]
    fn mixed_line_endings_are_reported_and_normalizable() {
        assert_eq!(inspect_line_endings("a\r\nb\nc"), LineEndings::Mixed);
        assert_eq!(
            normalize_line_endings("a\r\nb\nc", LineEndings::Crlf),
            "a\r\nb\r\nc"
        );
    }

    #[test]
    fn aliases_resolve_to_canonical_encoding() {
        assert_eq!(
            EncodingKind::for_label("sjis")
                .expect("alias")
                .canonical_name(),
            "Shift_JIS"
        );
    }
}
