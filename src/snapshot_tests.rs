use super::*;
use crate::model::{GraphNode, NodeId, NodeProvenance, RepoId, RepoNode};
use std::sync::{Arc, Barrier};
use std::thread;
use tempfile::TempDir;

/// Small populated snapshot used across the round-trip
/// tests. Carries one repo node, one node-provenance entry,
/// and one alias so the archived form exercises both
/// `node_provenance` (BTreeMap-with-NodeId-key) and the
/// alias overlay.
fn sample_snapshot() -> GraphSnapshot {
    let repo_id = RepoId::new("/r/.git");
    let mut snap = GraphSnapshot::empty();
    snap.nodes
        .push(GraphNode::Repo(RepoNode::new(repo_id.clone())));
    snap.node_provenance.insert(
        NodeId::Repo(repo_id.clone()),
        NodeProvenance {
            provider: "git".to_string(),
            freshness_epoch: Some(123),
        },
    );
    snap.aliases
        .insert(NodeId::Repo(repo_id), "myrepo".to_string());
    snap.canonicalize();
    snap
}

/// Drop into an `(_temp, path)` pair the rest of the suite
/// uses uniformly. The `_temp` binding keeps the temp dir
/// alive for the test's lifetime; `path` points at the
/// to-be-written snapshot inside it.
fn fresh_path(name: &str) -> (TempDir, PathBuf) {
    let temp = TempDir::new().expect("temp dir");
    let path = temp.path().join(name);
    (temp, path)
}

#[test]
fn header_round_trips_through_bytes() {
    let header = Header::new(1234);
    let bytes = header.to_bytes();
    let parsed = Header::parse(&bytes).expect("parse fresh header");
    assert_eq!(parsed, header);
    assert_eq!(parsed.magic, MAGIC);
    assert_eq!(parsed.format_version, FORMAT_VERSION);
    assert_eq!(parsed.payload_len, 1234);
    assert_eq!(parsed.reserved, [0u8; RESERVED_LEN]);
}

#[test]
fn header_rejects_short_buffer() {
    let err = Header::parse(&[0u8; 16]).expect_err("short header must error");
    assert!(matches!(
        err,
        SnapshotError::Truncated {
            needed: HEADER_LEN,
            actual: 16
        }
    ));
}

#[test]
fn header_rejects_wrong_magic() {
    let mut bytes = Header::new(0).to_bytes();
    bytes[0..8].copy_from_slice(b"OTHERMAG");
    let err = Header::parse(&bytes).expect_err("wrong magic must error");
    match err {
        SnapshotError::WrongMagic { expected, found } => {
            assert_eq!(expected, MAGIC);
            assert_eq!(&found, b"OTHERMAG");
        }
        other => panic!("expected WrongMagic, got {other:?}"),
    }
}

#[test]
fn header_rejects_future_format_version() {
    let mut bytes = Header::new(0).to_bytes();
    bytes[8..12].copy_from_slice(&(FORMAT_VERSION + 1).to_le_bytes());
    let err = Header::parse(&bytes).expect_err("future version must error");
    assert!(matches!(
        err,
        SnapshotError::IncompatibleVersion { expected, found }
            if expected == FORMAT_VERSION && found == FORMAT_VERSION + 1
    ));
}

#[test]
fn header_rejects_past_format_version() {
    let mut bytes = Header::new(0).to_bytes();
    bytes[8..12].copy_from_slice(&0u32.to_le_bytes());
    let err = Header::parse(&bytes).expect_err("zero version must error");
    assert!(matches!(
        err,
        SnapshotError::IncompatibleVersion { expected, found }
            if expected == FORMAT_VERSION && found == 0
    ));
}

#[test]
fn write_atomic_then_open_mmap_round_trips() {
    let (_temp, path) = fresh_path("graph.bin");
    let snap = sample_snapshot();
    write_atomic(&path, &snap).expect("write");
    let handle = open_mmap(&path).expect("open");
    assert_eq!(handle.header().format_version, FORMAT_VERSION);
    assert_eq!(handle.header().payload_len as usize, handle.payload().len());

    // Archived borrowing must produce a value structurally
    // equal to the original after `deserialize_owned`.
    let decoded = deserialize_owned(&handle).expect("deserialize");
    let mut decoded = decoded;
    decoded.canonicalize();
    assert_eq!(decoded, snap);
}

#[test]
fn open_mmap_rejects_corrupted_payload_byte() {
    let (_temp, path) = fresh_path("graph.bin");
    let snap = sample_snapshot();
    write_atomic(&path, &snap).expect("write");

    // Zero out the last 64 bytes of the payload. rkyv lays
    // out archives with the root struct + its relative
    // pointers near the trailing edge, so a wide
    // contiguous zero stripe there overwrites the
    // structural pointers / length prefixes that bytecheck
    // validates. Header is left intact so the failure
    // comes from `bytecheck`, not the header parser.
    let mut bytes = std::fs::read(&path).expect("read written file");
    let zero_start = bytes.len() - 64;
    for byte in &mut bytes[zero_start..] {
        *byte = 0;
    }
    std::fs::write(&path, &bytes).expect("rewrite");

    let err = open_mmap(&path).expect_err("corrupted payload must error");
    assert!(
        matches!(err, SnapshotError::Validate(_)),
        "expected Validate, got {err:?}"
    );
}

#[test]
fn write_atomic_failure_mid_write_leaves_target_untouched() {
    // Pre-populate `path` with a known-good snapshot, then
    // simulate a crash mid-write by writing a tmp file
    // manually without renaming. The target must still
    // contain the original snapshot — atomic-rename's whole
    // purpose.
    let (_temp, path) = fresh_path("graph.bin");
    let snap_a = sample_snapshot();
    write_atomic(&path, &snap_a).expect("write initial");

    let original_bytes = std::fs::read(&path).expect("read initial");
    let tmp = tmp_path_for(&path);
    {
        let mut tmp_file = std::fs::File::create(&tmp).expect("create tmp");
        tmp_file
            .write_all(b"partial garbage from simulated crash")
            .expect("write partial");
        // No rename. Drop the file handle.
    }

    let post_bytes = std::fs::read(&path).expect("read after partial");
    assert_eq!(
        post_bytes, original_bytes,
        "target file must be untouched after a tmp-only failed write"
    );
    // The handle should still open + validate cleanly.
    let handle = open_mmap(&path).expect("reopen");
    assert_eq!(handle.header().format_version, FORMAT_VERSION);
}

#[test]
fn from_bytes_round_trips_a_serialized_snapshot() {
    // The daemon-socket consumer path receives the
    // serialized bytes verbatim from the daemon and decodes
    // them in-memory (no detour through a tmp file).
    // `from_bytes` is the helper that path uses; round-trip
    // it against the same fixture the file-based path uses.
    let snap = sample_snapshot();
    let bytes = serialize_to_bytes(&snap).expect("serialize");
    let mut decoded = from_bytes(&bytes).expect("from_bytes");
    decoded.canonicalize();
    assert_eq!(decoded, snap);
}

#[test]
fn from_bytes_rejects_short_buffer() {
    let bytes = [0u8; 8];
    let err = from_bytes(&bytes).expect_err("short buffer must error");
    assert!(matches!(
        err,
        SnapshotError::Truncated {
            needed: HEADER_LEN,
            actual: 8
        }
    ));
}

#[test]
fn from_bytes_rejects_wrong_magic() {
    let mut bytes = serialize_to_bytes(&sample_snapshot()).expect("serialize");
    bytes[0..8].copy_from_slice(b"OTHERMAG");
    let err = from_bytes(&bytes).expect_err("wrong magic must error");
    assert!(matches!(err, SnapshotError::WrongMagic { .. }));
}

#[test]
fn concurrent_reader_holds_old_snapshot_across_writer_rename() {
    // Validates the inode-liveness contract from ADR 0083
    // §"Atomicity": a reader holding a mmap of the old file
    // continues to see the old data after a writer
    // atomic-renames a new file over the path.
    let (_temp, path) = fresh_path("graph.bin");
    let snap_a = sample_snapshot();
    write_atomic(&path, &snap_a).expect("write A");

    // Reader A maps the file and snapshots a byte we expect
    // to survive across a concurrent writer's rename.
    let reader_a = open_mmap(&path).expect("open A");
    let a_first_byte = reader_a.payload()[0];

    // Writer races a fresh snapshot (different in-memory
    // shape so the bytes differ) into the same path.
    let mut snap_b = sample_snapshot();
    // Mutate so the resulting archive bytes differ from A.
    snap_b
        .aliases
        .insert(NodeId::Repo(RepoId::new("/r/.git")), "renamed".to_string());
    let barrier = Arc::new(Barrier::new(2));
    let writer_barrier = Arc::clone(&barrier);
    let writer_path = path.clone();
    let writer = thread::spawn(move || {
        writer_barrier.wait();
        write_atomic(&writer_path, &snap_b).expect("write B");
    });
    barrier.wait();
    writer.join().expect("writer");

    // Reader A's mmap still reflects the original bytes —
    // the rename swapped the inode behind the path but A's
    // mapping is on the old inode.
    assert_eq!(
        reader_a.payload()[0],
        a_first_byte,
        "reader's mmap must keep seeing the pre-rename snapshot"
    );

    // A fresh open picks up the new snapshot.
    let reader_b = open_mmap(&path).expect("reopen post-write");
    let decoded_b = deserialize_owned(&reader_b).expect("deserialize B");
    let aliased = decoded_b.aliases.get(&NodeId::Repo(RepoId::new("/r/.git")));
    assert_eq!(aliased, Some("renamed"));
}
