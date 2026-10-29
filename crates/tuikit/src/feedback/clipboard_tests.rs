//! The OSC 52 sequence, captured through the sink, and the encoder's
//! RFC 4648 test vectors.

use std::io::{self, Write};

use super::*;

/// The sequence `copy` writes for `text`, read back from a `Vec<u8>`.
fn copied(text: &str) -> Vec<u8> {
    let mut sink = Vec::new();
    copy(&mut sink, text).expect("a Vec takes every write");
    sink
}

#[test]
fn copy_writes_the_osc52_sequence_to_the_sink() {
    assert_eq!(copied("foo"), b"\x1b]52;c;Zm9v\x07");
}

#[test]
fn an_empty_copy_is_a_well_formed_sequence() {
    assert_eq!(copied(""), b"\x1b]52;c;\x07");
}

#[test]
fn text_is_encoded_as_utf8() {
    // "caf\u{e9}" is 63 61 66 C3 A9 in UTF-8.
    assert_eq!(copied("caf\u{e9}"), b"\x1b]52;c;Y2Fmw6k=\x07");
}

#[test]
fn copy_works_through_a_trait_object_and_flushes() {
    /// Counts flushes, so the test sees the sequence is pushed out.
    #[derive(Default)]
    struct Flushes {
        bytes: Vec<u8>,
        flushed: usize,
    }
    impl Write for Flushes {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.bytes.extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            self.flushed += 1;
            Ok(())
        }
    }
    let mut sink = Flushes::default();
    let dyn_sink: &mut dyn Write = &mut sink;
    copy(dyn_sink, "fo").unwrap();
    assert_eq!(sink.bytes, b"\x1b]52;c;Zm8=\x07");
    assert_eq!(sink.flushed, 1);
}

#[test]
fn a_failing_sink_reports_its_error() {
    struct Broken;
    impl Write for Broken {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::Error::from(io::ErrorKind::BrokenPipe))
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let error = copy(&mut Broken, "foo").unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
}

#[test]
fn base64_matches_the_rfc_4648_vectors() {
    let vectors: [(&[u8], &str); 7] = [
        (b"", ""),
        (b"f", "Zg=="),
        (b"fo", "Zm8="),
        (b"foo", "Zm9v"),
        (b"foob", "Zm9vYg=="),
        (b"fooba", "Zm9vYmE="),
        (b"foobar", "Zm9vYmFy"),
    ];
    for (input, encoded) in vectors {
        assert_eq!(base64(input), encoded, "{input:?}");
    }
    assert_eq!(base64("\u{e9}".as_bytes()), "w6k=");
}
