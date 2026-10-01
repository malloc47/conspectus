#![allow(dead_code)]

pub mod fixtures;
pub mod replay;

/// Replace wall-clock `freshness_epoch` integer values in serialized
/// JSON / table output with a stable `"<redacted>"` placeholder so
/// snapshot diffs stay deterministic regardless of when the test
/// runs. Targets the freshness fields on `NodeProvenance` and
/// `SourceMetadata`; the provider key itself (alongside the redacted
/// epoch) still anchors the row in the diff.
pub fn redact_freshness_epoch(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for line in text.split_inclusive('\n') {
        if let Some(idx) = line.find("\"freshness_epoch\":") {
            let (head, tail) = line.split_at(idx + "\"freshness_epoch\":".len());
            let trailing = tail.find([',', '\n', '}']).map_or("", |i| &tail[i..]);
            out.push_str(head);
            out.push_str(" \"<redacted>\"");
            out.push_str(trailing);
            continue;
        }
        out.push_str(line);
    }
    out
}
