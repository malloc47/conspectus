//! Copying to the system clipboard through the terminal (OSC 52).
//!
//! A program inside a terminal copies by writing an OSC 52 sequence
//! (`ESC ] 52 ; c ; <base64> BEL`) to the terminal, which puts the decoded
//! text on its clipboard. It needs no platform library and works over a
//! remote shell; a terminal that does not support it ignores the
//! sequence, and nothing comes back to say whether it was honored, so a
//! host still tells the user it copied.
//!
//! The framework writes to nothing of its own: [`copy`] writes the
//! sequence to the sink it is given. A host passes the terminal's output;
//! a test passes a `Vec<u8>` and reads the bytes.
//!
//! ```
//! use tuikit::feedback::clipboard;
//!
//! let mut sink = Vec::new();
//! clipboard::copy(&mut sink, "foo").unwrap();
//! assert_eq!(sink, b"\x1b]52;c;Zm9v\x07");
//! ```

use std::io::{self, Write};

/// Write the OSC 52 sequence that puts `text` on the clipboard to `sink`,
/// and flush it. The sequence ends with `BEL`, the terminator terminals
/// and multiplexers document for OSC 52. An empty `text` writes a valid
/// sequence that clears the clipboard.
pub fn copy<W: Write + ?Sized>(sink: &mut W, text: &str) -> io::Result<()> {
    sink.write_all(b"\x1b]52;c;")?;
    sink.write_all(base64(text.as_bytes()).as_bytes())?;
    sink.write_all(b"\x07")?;
    sink.flush()
}

/// The standard base64 alphabet (RFC 4648, section 4), with padding.
/// Only the encoder is needed, so the framework carries these few lines
/// rather than a crate.
fn base64(input: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let symbol = |index: u8| char::from(ALPHABET[usize::from(index)]);
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    let (chunks, rest) = input.as_chunks::<3>();
    for &[b0, b1, b2] in chunks {
        out.push(symbol(b0 >> 2));
        out.push(symbol(((b0 & 0b11) << 4) | (b1 >> 4)));
        out.push(symbol(((b1 & 0b1111) << 2) | (b2 >> 6)));
        out.push(symbol(b2 & 0b11_1111));
    }
    match *rest {
        [b0] => {
            out.push(symbol(b0 >> 2));
            out.push(symbol((b0 & 0b11) << 4));
            out.push_str("==");
        }
        [b0, b1] => {
            out.push(symbol(b0 >> 2));
            out.push(symbol(((b0 & 0b11) << 4) | (b1 >> 4)));
            out.push(symbol((b1 & 0b1111) << 2));
            out.push('=');
        }
        _ => {}
    }
    out
}

#[cfg(test)]
#[path = "clipboard_tests.rs"]
mod tests;
