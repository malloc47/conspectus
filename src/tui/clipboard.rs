//! Clipboard write path for the TUI (ADR 0056).
//!
//! Emits an OSC 52 escape sequence on stdout so the host terminal (or
//! tmux, when `set-clipboard on` is configured) writes the payload to
//! the system clipboard. No new dependencies: the base64 encoder is
//! hand-rolled in this module per ADR 0056.
//!
//! OSC 52 is fire-and-forget. Terminals that don't implement it drop
//! the sequence silently. Callers should still surface a "copied"
//! toast — this matches the behavior of every other tool that writes
//! the clipboard via terminal escapes (vim's `+y`, tmux copy-mode,
//! etc.).

use std::io::{self, Write};

/// Write `text` to the system clipboard via an OSC 52 sequence on
/// stdout. Returns the underlying I/O error if the write fails (rare
/// while the TUI owns the terminal).
pub fn copy_to_clipboard(text: &str) -> io::Result<()> {
    let mut stdout = io::stdout().lock();
    write_osc52(&mut stdout, text)?;
    stdout.flush()
}

/// Emit the OSC 52 sequence into an arbitrary writer. Exposed so unit
/// tests can assert on the produced byte sequence without touching
/// real stdout.
fn write_osc52<W: Write>(w: &mut W, text: &str) -> io::Result<()> {
    let encoded = base64_encode(text.as_bytes());
    w.write_all(b"\x1b]52;c;")?;
    w.write_all(encoded.as_bytes())?;
    w.write_all(b"\x07")
}

/// Standard-alphabet (RFC 4648 §4) base64 encoder. Only the encoder
/// is needed for OSC 52 payloads; see ADR 0056 for why the `base64`
/// crate is not adopted.
fn base64_encode(input: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    let mut chunks = input.chunks_exact(3);
    for chunk in chunks.by_ref() {
        let b0 = chunk[0];
        let b1 = chunk[1];
        let b2 = chunk[2];
        out.push(ALPHABET[(b0 >> 2) as usize] as char);
        out.push(ALPHABET[(((b0 & 0b11) << 4) | (b1 >> 4)) as usize] as char);
        out.push(ALPHABET[(((b1 & 0b1111) << 2) | (b2 >> 6)) as usize] as char);
        out.push(ALPHABET[(b2 & 0b111111) as usize] as char);
    }
    let rem = chunks.remainder();
    match rem.len() {
        1 => {
            let b0 = rem[0];
            out.push(ALPHABET[(b0 >> 2) as usize] as char);
            out.push(ALPHABET[((b0 & 0b11) << 4) as usize] as char);
            out.push('=');
            out.push('=');
        }
        2 => {
            let b0 = rem[0];
            let b1 = rem[1];
            out.push(ALPHABET[(b0 >> 2) as usize] as char);
            out.push(ALPHABET[(((b0 & 0b11) << 4) | (b1 >> 4)) as usize] as char);
            out.push(ALPHABET[((b1 & 0b1111) << 2) as usize] as char);
            out.push('=');
        }
        _ => {}
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_empty() {
        assert_eq!(base64_encode(b""), "");
    }

    #[test]
    fn base64_one_byte_pads_two_equals() {
        // "f" -> "Zg==" per RFC 4648 §10 test vectors.
        assert_eq!(base64_encode(b"f"), "Zg==");
    }

    #[test]
    fn base64_two_bytes_pads_one_equals() {
        // "fo" -> "Zm8=" per RFC 4648 §10 test vectors.
        assert_eq!(base64_encode(b"fo"), "Zm8=");
    }

    #[test]
    fn base64_three_bytes_no_padding() {
        // "foo" -> "Zm9v" per RFC 4648 §10 test vectors.
        assert_eq!(base64_encode(b"foo"), "Zm9v");
    }

    #[test]
    fn base64_rfc4648_vectors() {
        // Remaining RFC 4648 §10 vectors. Covers the chunked-and-
        // remainder path, including a 6-byte input that hits both
        // chunked and exact-multiple paths.
        assert_eq!(base64_encode(b"foob"), "Zm9vYg==");
        assert_eq!(base64_encode(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn base64_unicode_is_utf8_encoded_first() {
        // "é" is 0xC3 0xA9 in UTF-8 -> "w6k=".
        assert_eq!(base64_encode("é".as_bytes()), "w6k=");
    }

    #[test]
    fn osc52_emits_expected_byte_sequence() {
        // ESC ] 52 ; c ; <base64> BEL
        let mut buf = Vec::new();
        write_osc52(&mut buf, "foo").unwrap();
        assert_eq!(buf, b"\x1b]52;c;Zm9v\x07");
    }

    #[test]
    fn osc52_handles_empty_payload() {
        // Empty input still produces a well-formed sequence — terminals
        // accept an empty c-buffer write as "clear the clipboard," so
        // the toast never claims a copy that didn't happen.
        let mut buf = Vec::new();
        write_osc52(&mut buf, "").unwrap();
        assert_eq!(buf, b"\x1b]52;c;\x07");
    }

    #[test]
    fn osc52_unicode_payload_round_trips_to_utf8_base64() {
        let mut buf = Vec::new();
        write_osc52(&mut buf, "café").unwrap();
        // "café" is 63 61 66 C3 A9 in UTF-8 -> "Y2Fmw6k=".
        assert_eq!(buf, b"\x1b]52;c;Y2Fmw6k=\x07");
    }
}
