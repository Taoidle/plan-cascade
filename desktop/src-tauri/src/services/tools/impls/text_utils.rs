//! Shared text-detection utilities
//!
//! Contains helper functions for detecting binary vs text content and
//! decoding file bytes. Shared by `ReadTool` and `EditTool`.

/// Logical text encoding used by read/write/edit tools.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TextEncoding {
    Utf8,
    Utf16Le,
    Utf16Be,
}

/// Preferred line ending style of an existing file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LineEnding {
    Lf,
    Crlf,
}

/// Text serialization format metadata preserved by edit/write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TextFormat {
    pub encoding: TextEncoding,
    pub has_bom: bool,
    pub line_ending: LineEnding,
}

impl Default for TextFormat {
    fn default() -> Self {
        Self {
            encoding: TextEncoding::Utf8,
            has_bom: false,
            line_ending: LineEnding::Lf,
        }
    }
}

/// Decoded text payload with fidelity metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DecodedText {
    pub text: String,
    pub lossy: bool,
    pub format: TextFormat,
}

/// Check whether a file extension is known to be a text format.
pub(crate) fn is_likely_text_extension(ext: &str) -> bool {
    matches!(
        ext,
        "txt"
            | "md"
            | "markdown"
            | "rst"
            | "json"
            | "jsonl"
            | "yaml"
            | "yml"
            | "toml"
            | "ini"
            | "cfg"
            | "conf"
            | "lock"
            | "env"
            | "gitignore"
            | "gitattributes"
            | "py"
            | "rs"
            | "ts"
            | "tsx"
            | "js"
            | "jsx"
            | "java"
            | "kt"
            | "go"
            | "c"
            | "h"
            | "cpp"
            | "hpp"
            | "cs"
            | "rb"
            | "php"
            | "swift"
            | "scala"
            | "sql"
            | "sh"
            | "bash"
            | "ps1"
            | "zsh"
            | "fish"
            | "xml"
            | "html"
            | "htm"
            | "css"
            | "scss"
            | "less"
            | "svg"
            | "vue"
            | "svelte"
    )
}

/// Heuristic check: does the byte buffer look like binary data?
///
/// Scans up to the first 4 KiB.  Returns `true` when a NUL byte is
/// found or when >30 % of bytes are non-text-like.
pub(crate) fn is_probably_binary(bytes: &[u8]) -> bool {
    if bytes.is_empty() {
        return false;
    }
    let sample_len = bytes.len().min(4096);
    let sample = &bytes[..sample_len];
    if sample.contains(&0) {
        return true;
    }
    let mut suspicious = 0usize;
    for b in sample {
        let is_text_like = matches!(*b, 0x09 | 0x0A | 0x0D | 0x20..=0x7E);
        if !is_text_like {
            suspicious += 1;
        }
    }
    (suspicious as f64 / sample_len as f64) > 0.30
}

fn detect_line_ending(text: &str) -> LineEnding {
    if text.contains("\r\n") {
        LineEnding::Crlf
    } else {
        LineEnding::Lf
    }
}

fn normalize_to_lf(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

fn apply_line_ending(text: &str, line_ending: LineEnding) -> String {
    let normalized = normalize_to_lf(text);
    match line_ending {
        LineEnding::Lf => normalized,
        LineEnding::Crlf => normalized.replace('\n', "\r\n"),
    }
}

fn decode_utf16(bytes: &[u8], little_endian: bool) -> (String, bool) {
    let mut lossy = false;
    let mut units = Vec::with_capacity(bytes.len() / 2);

    let mut chunks = bytes.chunks_exact(2);
    for pair in &mut chunks {
        let unit = if little_endian {
            u16::from_le_bytes([pair[0], pair[1]])
        } else {
            u16::from_be_bytes([pair[0], pair[1]])
        };
        units.push(unit);
    }

    if !chunks.remainder().is_empty() {
        lossy = true;
    }

    match String::from_utf16(&units) {
        Ok(text) => (text, lossy),
        Err(_) => {
            let text: String = std::char::decode_utf16(units)
                .map(|r| r.unwrap_or('\u{FFFD}'))
                .collect();
            (text, true)
        }
    }
}

/// Try to decode raw bytes as text with format metadata.
///
/// Returns `Some((text, lossy))` where `lossy` is `true` when
/// `from_utf8_lossy` was used.  Returns `None` when the file is
/// determined to be binary.
pub(crate) fn decode_text_with_format(bytes: &[u8], ext: &str) -> Option<DecodedText> {
    // UTF-16 LE with BOM
    if bytes.starts_with(&[0xFF, 0xFE]) {
        let (text, lossy) = decode_utf16(&bytes[2..], true);
        return Some(DecodedText {
            format: TextFormat {
                encoding: TextEncoding::Utf16Le,
                has_bom: true,
                line_ending: detect_line_ending(&text),
            },
            text,
            lossy,
        });
    }

    // UTF-16 BE with BOM
    if bytes.starts_with(&[0xFE, 0xFF]) {
        let (text, lossy) = decode_utf16(&bytes[2..], false);
        return Some(DecodedText {
            format: TextFormat {
                encoding: TextEncoding::Utf16Be,
                has_bom: true,
                line_ending: detect_line_ending(&text),
            },
            text,
            lossy,
        });
    }

    // UTF-8 with BOM
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        let body = &bytes[3..];
        return match std::str::from_utf8(body) {
            Ok(text) => Some(DecodedText {
                format: TextFormat {
                    encoding: TextEncoding::Utf8,
                    has_bom: true,
                    line_ending: detect_line_ending(text),
                },
                text: text.to_string(),
                lossy: false,
            }),
            Err(_) => Some(DecodedText {
                format: TextFormat {
                    encoding: TextEncoding::Utf8,
                    has_bom: true,
                    line_ending: detect_line_ending(&String::from_utf8_lossy(body)),
                },
                text: String::from_utf8_lossy(body).into_owned(),
                lossy: true,
            }),
        };
    }

    match std::str::from_utf8(bytes) {
        Ok(text) => Some(DecodedText {
            format: TextFormat {
                encoding: TextEncoding::Utf8,
                has_bom: false,
                line_ending: detect_line_ending(text),
            },
            text: text.to_string(),
            lossy: false,
        }),
        Err(_) => {
            if is_likely_text_extension(ext) || !is_probably_binary(bytes) {
                let text = String::from_utf8_lossy(bytes).into_owned();
                Some(DecodedText {
                    format: TextFormat {
                        encoding: TextEncoding::Utf8,
                        has_bom: false,
                        line_ending: detect_line_ending(&text),
                    },
                    text,
                    lossy: true,
                })
            } else {
                None
            }
        }
    }
}

/// Encode text using a previously detected file format.
pub(crate) fn encode_text_with_format(text: &str, format: TextFormat) -> Vec<u8> {
    let normalized = apply_line_ending(text, format.line_ending);

    match format.encoding {
        TextEncoding::Utf8 => {
            let mut out = Vec::new();
            if format.has_bom {
                out.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
            }
            out.extend_from_slice(normalized.as_bytes());
            out
        }
        TextEncoding::Utf16Le => {
            let mut out = Vec::new();
            if format.has_bom {
                out.extend_from_slice(&[0xFF, 0xFE]);
            }
            for unit in normalized.encode_utf16() {
                out.extend_from_slice(&unit.to_le_bytes());
            }
            out
        }
        TextEncoding::Utf16Be => {
            let mut out = Vec::new();
            if format.has_bom {
                out.extend_from_slice(&[0xFE, 0xFF]);
            }
            for unit in normalized.encode_utf16() {
                out.extend_from_slice(&unit.to_be_bytes());
            }
            out
        }
    }
}

/// Backward-compatible helper for read tool.
pub(crate) fn decode_read_text(bytes: &[u8], ext: &str) -> Option<(String, bool)> {
    decode_text_with_format(bytes, ext).map(|d| (d.text, d.lossy))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_common_text_extensions() {
        assert!(is_likely_text_extension("rs"));
        assert!(is_likely_text_extension("py"));
        assert!(is_likely_text_extension("ts"));
        assert!(is_likely_text_extension("json"));
        assert!(!is_likely_text_extension("png"));
        assert!(!is_likely_text_extension("exe"));
    }

    #[test]
    fn test_binary_detection() {
        assert!(!is_probably_binary(b""));
        assert!(!is_probably_binary(b"hello world\n"));
        assert!(is_probably_binary(b"\x00\x01\x02\x03"));
    }

    #[test]
    fn test_decode_text() {
        let (text, lossy) = decode_read_text(b"hello", "txt").unwrap();
        assert_eq!(text, "hello");
        assert!(!lossy);
    }

    #[test]
    fn test_decode_utf8_bom_preserved_metadata() {
        let bytes = [0xEF, 0xBB, 0xBF, b'h', b'i', b'\r', b'\n'];
        let decoded = decode_text_with_format(&bytes, "txt").unwrap();
        assert_eq!(decoded.text, "hi\r\n");
        assert!(!decoded.lossy);
        assert_eq!(decoded.format.encoding, TextEncoding::Utf8);
        assert!(decoded.format.has_bom);
        assert_eq!(decoded.format.line_ending, LineEnding::Crlf);
    }

    #[test]
    fn test_decode_utf16le_with_bom() {
        let mut bytes = vec![0xFF, 0xFE];
        for u in "hi\r\n".encode_utf16() {
            bytes.extend_from_slice(&u.to_le_bytes());
        }
        let decoded = decode_text_with_format(&bytes, "txt").unwrap();
        assert_eq!(decoded.text, "hi\r\n");
        assert_eq!(decoded.format.encoding, TextEncoding::Utf16Le);
        assert!(decoded.format.has_bom);
        assert_eq!(decoded.format.line_ending, LineEnding::Crlf);
    }

    #[test]
    fn test_encode_text_with_format_utf16le_crlf() {
        let format = TextFormat {
            encoding: TextEncoding::Utf16Le,
            has_bom: true,
            line_ending: LineEnding::Crlf,
        };
        let bytes = encode_text_with_format("a\nb\n", format);
        assert!(bytes.starts_with(&[0xFF, 0xFE]));

        let decoded = decode_text_with_format(&bytes, "txt").unwrap();
        assert_eq!(decoded.text, "a\r\nb\r\n");
        assert_eq!(decoded.format.encoding, TextEncoding::Utf16Le);
        assert!(decoded.format.has_bom);
        assert_eq!(decoded.format.line_ending, LineEnding::Crlf);
    }

    #[test]
    fn test_decode_binary_returns_none() {
        // Mix of null bytes and high bytes that fail UTF-8 validation
        // and trigger the binary detection heuristic
        let mut binary = vec![0xFFu8; 100];
        binary[0] = 0x80; // invalid UTF-8 start byte
        assert!(decode_read_text(&binary, "bin").is_none());
    }
}
