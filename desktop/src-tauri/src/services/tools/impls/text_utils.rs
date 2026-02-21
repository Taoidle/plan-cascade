//! Shared text-detection utilities
//!
//! Contains helper functions for detecting binary vs text content and
//! decoding file bytes. Shared by `ReadTool` and `EditTool`.

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

/// Try to decode raw bytes as text.
///
/// Returns `Some((text, lossy))` where `lossy` is `true` when
/// `from_utf8_lossy` was used.  Returns `None` when the file is
/// determined to be binary.
pub(crate) fn decode_read_text(bytes: &[u8], ext: &str) -> Option<(String, bool)> {
    match std::str::from_utf8(bytes) {
        Ok(text) => Some((text.to_string(), false)),
        Err(_) => {
            if is_likely_text_extension(ext) || !is_probably_binary(bytes) {
                Some((String::from_utf8_lossy(bytes).into_owned(), true))
            } else {
                None
            }
        }
    }
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
    fn test_decode_binary_returns_none() {
        // Mix of null bytes and high bytes that fail UTF-8 validation
        // and trigger the binary detection heuristic
        let mut binary = vec![0xFFu8; 100];
        binary[0] = 0x80; // invalid UTF-8 start byte
        assert!(decode_read_text(&binary, "bin").is_none());
    }
}
