//! Text document load/save adapters with encoding and line-ending preservation.
#![allow(
    clippy::double_must_use,
    clippy::map_unwrap_or,
    clippy::missing_errors_doc,
    clippy::must_use_candidate
    ,
    clippy::match_same_arms,
    clippy::single_match_else,
    clippy::trivially_copy_pass_by_ref
)]

use std::{
    borrow::Cow,
    fs::File,
    io::{self, Write},
    path::{Path, PathBuf},
};

use chardetng::EncodingDetector;
use config_core::LargeFileSettings;
use encoding_rs::{Encoding, UTF_8};
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodePolicy {
    Strict,
    Replace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LineEndings {
    None,
    Lf,
    Crlf,
    Mixed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EncodingKind {
    Utf8,
    Utf16Le,
    Utf16Be,
    Legacy(String),
}

impl EncodingKind {
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

    #[must_use]
    pub fn canonical_name(&self) -> &str {
        match self {
            Self::Utf8 => "UTF-8",
            Self::Utf16Le => "UTF-16LE",
            Self::Utf16Be => "UTF-16BE",
            Self::Legacy(label) => label,
        }
    }

    fn encoding_rs(&self) -> Result<&'static Encoding, EncodingError> {
        match self {
            Self::Utf8 => Ok(UTF_8),
            Self::Legacy(label) => Encoding::for_label(label.as_bytes()).ok_or_else(|| {
                EncodingError::UnsupportedEncoding {
                    label: label.clone(),
                }
            }),
            Self::Utf16Le | Self::Utf16Be => Err(EncodingError::UnsupportedEncoding {
                label: self.canonical_name().to_owned(),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedText {
    pub text: String,
    pub encoding: EncodingKind,
    pub had_bom: bool,
    pub had_replacements: bool,
    pub line_endings: LineEndings,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum EncodingError {
    #[error("unsupported encoding label: {label}")]
    UnsupportedEncoding { label: String },
    #[error("input is malformed {encoding} data")]
    MalformedInput { encoding: String },
    #[error("text contains characters that cannot be represented in {encoding}")]
    UnmappableText { encoding: String },
}

#[derive(Debug, Error)]
pub enum DocumentError {
    #[error("io error: {0}")]
    Io(#[from] io::Error),
    #[error(transparent)]
    Encoding(#[from] EncodingError),
    #[error("write target parent directory does not exist: {path}")]
    MissingParent { path: PathBuf },
}

pub type DocumentOpenError = DocumentError;
pub type DocumentSaveError = DocumentError;
pub type DocumentEncodingError = EncodingError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentLoadOptions {
    pub fallback_encoding: EncodingKind,
    pub malformed_input_policy: DecodePolicy,
    pub large_file_settings: LargeFileSettings,
}

impl Default for DocumentLoadOptions {
    fn default() -> Self {
        Self {
            fallback_encoding: EncodingKind::Utf8,
            malformed_input_policy: DecodePolicy::Strict,
            large_file_settings: LargeFileSettings::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DocumentSaveOptions {
    pub encoding: EncodingKind,
    pub with_bom: bool,
    pub line_endings: LineEndings,
    pub malformed_input_policy: DecodePolicy,
}

impl Default for DocumentSaveOptions {
    fn default() -> Self {
        Self {
            encoding: EncodingKind::Utf8,
            with_bom: false,
            line_endings: LineEndings::Lf,
            malformed_input_policy: DecodePolicy::Strict,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextDocument {
    pub path: PathBuf,
    pub text: String,
    pub encoding: EncodingKind,
    pub had_bom: bool,
    pub line_endings: LineEndings,
    pub large_file: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveOutcome {
    pub path: PathBuf,
    pub bytes_written: usize,
    pub replaced_existing: bool,
}

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

#[must_use]
pub fn normalize_line_endings(text: &str, target: LineEndings) -> String {
    let separator = match target {
        LineEndings::Crlf => "\r\n",
        LineEndings::Lf => "\n",
        LineEndings::None | LineEndings::Mixed => return text.to_owned(),
    };
    text.replace("\r\n", "\n").replace('\n', separator)
}

pub fn decode_text_document(
    bytes: &[u8],
    fallback: &EncodingKind,
    policy: DecodePolicy,
) -> Result<DecodedText, EncodingError> {
    let (encoding, had_bom) = detect_encoding(bytes, fallback)?;
    let payload = match &encoding {
        EncodingKind::Utf8 if had_bom => &bytes[3..],
        EncodingKind::Utf16Le if had_bom => &bytes[2..],
        EncodingKind::Utf16Be if had_bom => &bytes[2..],
        _ => bytes,
    };
    let (text, had_replacements) = decode_as(payload, &encoding)?;
    if had_replacements && policy == DecodePolicy::Strict {
        return Err(EncodingError::MalformedInput {
            encoding: encoding.canonical_name().to_owned(),
        });
    }
    Ok(DecodedText {
        line_endings: inspect_line_endings(&text),
        text,
        encoding,
        had_bom,
        had_replacements,
    })
}

pub fn encode_text_document(
    text: &str,
    encoding: &EncodingKind,
    with_bom: bool,
    policy: DecodePolicy,
) -> Result<Vec<u8>, EncodingError> {
    match encoding {
        EncodingKind::Utf8 => {
            let mut output = Vec::with_capacity(text.len() + usize::from(with_bom) * 3);
            if with_bom {
                output.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
            }
            output.extend_from_slice(text.as_bytes());
            Ok(output)
        }
        EncodingKind::Utf16Le | EncodingKind::Utf16Be => {
            let mut output = Vec::with_capacity(text.len() * 2 + usize::from(with_bom) * 2);
            if with_bom {
                output.extend_from_slice(if *encoding == EncodingKind::Utf16Le {
                    &[0xFF, 0xFE]
                } else {
                    &[0xFE, 0xFF]
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

pub fn load_text_document(
    path: &Path,
    options: &DocumentLoadOptions,
) -> Result<TextDocument, DocumentError> {
    let bytes = std::fs::read(path)?;
    let decoded = decode_text_document(
        &bytes,
        &options.fallback_encoding,
        options.malformed_input_policy,
    )?;
    Ok(TextDocument {
        path: path.to_path_buf(),
        text: decoded.text,
        encoding: decoded.encoding,
        had_bom: decoded.had_bom,
        line_endings: decoded.line_endings,
        large_file: is_large_file(path, &options.large_file_settings),
    })
}

pub fn save_text_document(
    path: &Path,
    text: &str,
    options: &DocumentSaveOptions,
) -> Result<SaveOutcome, DocumentError> {
    let normalized = normalize_line_endings(text, options.line_endings);
    let bytes = encode_text_document(
        &normalized,
        &options.encoding,
        options.with_bom,
        options.malformed_input_policy,
    )?;
    save_bytes_atomically(path, &bytes)
}

pub(crate) fn save_bytes_atomically(path: &Path, bytes: &[u8]) -> Result<SaveOutcome, DocumentError> {
    save_bytes_atomically_with(path, bytes, |temp_path, destination| {
        temp_path
            .persist(destination)
            .map_err(|error| DocumentError::Io(error.error))
    })
}

pub(crate) fn save_bytes_atomically_with(
    path: &Path,
    bytes: &[u8],
    commit: impl FnOnce(tempfile::TempPath, &Path) -> Result<(), DocumentError>,
) -> Result<SaveOutcome, DocumentError> {
    let parent = path.parent().ok_or_else(|| DocumentError::MissingParent {
        path: path.to_path_buf(),
    })?;
    if !parent.exists() {
        return Err(DocumentError::MissingParent {
            path: parent.to_path_buf(),
        });
    }
    let replaced_existing = path.exists();
    let temp = NamedTempFile::new_in(parent)?;
    write_all_and_sync(temp.as_file(), bytes)?;
    let temp_path = temp.into_temp_path();
    commit(temp_path, path)?;
    Ok(SaveOutcome {
        path: path.to_path_buf(),
        bytes_written: bytes.len(),
        replaced_existing,
    })
}

pub(crate) fn write_all_and_sync(file: &File, bytes: &[u8]) -> Result<(), io::Error> {
    let mut writer = file;
    writer.write_all(bytes)?;
    writer.sync_all()
}

#[must_use]
pub(crate) fn is_large_file(path: &Path, settings: &LargeFileSettings) -> bool {
    std::fs::metadata(path)
        .map(|metadata| metadata.len() >= settings.threshold_bytes)
        .unwrap_or(false)
}

fn detect_encoding(bytes: &[u8], fallback: &EncodingKind) -> Result<(EncodingKind, bool), EncodingError> {
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return Ok((EncodingKind::Utf8, true));
    }
    if bytes.starts_with(&[0xFF, 0xFE]) {
        return Ok((EncodingKind::Utf16Le, true));
    }
    if bytes.starts_with(&[0xFE, 0xFF]) {
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
    if let EncodingKind::Legacy(_) = fallback {
        fallback.encoding_rs()?;
    }
    Ok((fallback.clone(), false))
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
        .map(|result| match result {
            Ok(character) => character,
            Err(_) => {
                had_errors = true;
                char::REPLACEMENT_CHARACTER
            }
        })
        .collect();
    (text, had_errors)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn utf_bom_round_trip() {
        for encoding in [
            EncodingKind::Utf8,
            EncodingKind::Utf16Le,
            EncodingKind::Utf16Be,
        ] {
            let bytes = encode_text_document("A日本", &encoding, true, DecodePolicy::Strict)
                .expect("encode");
            let decoded = decode_text_document(&bytes, &EncodingKind::Utf8, DecodePolicy::Strict)
                .expect("decode");
            assert_eq!(decoded.text, "A日本");
            assert_eq!(decoded.encoding, encoding);
            assert!(decoded.had_bom);
        }
    }

    #[test]
    fn strict_legacy_encoding_rejects_unmappable_text() {
        let encoding = EncodingKind::for_label("windows-1252").expect("encoding");
        let error = encode_text_document("日本語", &encoding, false, DecodePolicy::Strict)
            .expect_err("strict");
        assert!(matches!(error, EncodingError::UnmappableText { .. }));
    }

    #[test]
    fn atomic_save_preserves_existing_file_on_failure() {
        let dir = tempdir().expect("dir");
        let path = dir.path().join("note.txt");
        std::fs::write(&path, "old").expect("seed");
        let error = save_bytes_atomically_with(&path, b"new", |_temp_path, _destination| {
            Err(DocumentError::Io(io::Error::other("forced failure")))
        })
        .expect_err("failure");
        assert!(matches!(error, DocumentError::Io(_)));
        assert_eq!(std::fs::read_to_string(&path).expect("read"), "old");
    }
}
