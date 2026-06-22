//! On-disk snapshot format (ADR 0083).
//!
//! The daemon writes a single mmap-able binary artifact after
//! each successful per-class cycle; one-shot CLIs read the same
//! artifact when no daemon is reachable. The wire format is rkyv
//! 0.8 archives prefixed with a 32-byte fixed header.
//!
//! File layout:
//!
//! ```text
//! offset  size  field
//!   0      8    magic           = b"CONSPECT"
//!   8      4    format_version  = u32 (LE), starting at 1
//!  12      4    payload_len     = u32 (LE), bytes in archive
//!  16     16    reserved        = zero-filled
//! ```
//!
//! Atomicity is the standard POSIX `write tmp` + `rename` dance;
//! POSIX guarantees concurrent readers holding the old file's
//! inode keep seeing the old data until they drop the mapping.
//! There is no WAL, no locking. Version mismatches abort with a
//! typed error so the caller can fall through to a cold rebuild
//! per ADR 0082's "version mismatch → cold rebuild" policy.
//!
//! This module is library-only: P11-005 wires the daemon to call
//! [`write_atomic`], P11-007/008 wire the readers to call
//! [`open_mmap`] (or [`open_mmap_unvalidated`] when the caller
//! trusts the source — e.g. the socket-served bytes path).

use std::env;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use memmap2::Mmap;
use rkyv::rancor;

use crate::model::{ArchivedGraphSnapshot, GraphSnapshot};

/// File magic for the on-disk artifact. Eight ASCII bytes so the
/// header is hand-recognizable in `xxd`. Stable across format
/// versions; readers reject a wrong magic before they even look
/// at [`FORMAT_VERSION`].
pub const MAGIC: [u8; 8] = *b"CONSPECT";

/// Header byte count. The header is fixed-width so the payload
/// always begins at `HEADER_LEN` regardless of the payload's
/// internal layout.
pub const HEADER_LEN: usize = 32;

/// Bumped manually on any breaking model change (field
/// add/remove, enum variant add, rename). Readers refuse to
/// touch a file written with a mismatched version and fall
/// through to cold rebuild per ADR 0082.
pub const FORMAT_VERSION: u32 = 1;

/// Reserved-header byte count. Carved out so future additive
/// header changes (e.g. a `crc32c`, a feature-flag bitmap) can
/// land without bumping [`FORMAT_VERSION`].
const RESERVED_LEN: usize = 16;

/// Largest payload we will accept on the read side. The archive
/// itself is bounded by available memory at write time, but a
/// hostile or truncated file with a forged `payload_len` could
/// otherwise cause an unbounded allocation in `Vec::with_capacity`
/// (we currently don't pre-allocate, but the cap defends against
/// future code that might).
const MAX_PAYLOAD_LEN: usize = 1 << 30; // 1 GiB

/// Canonical on-disk location for the zero-copy snapshot artifact
/// per ADR 0083. Resolves under `$XDG_DATA_HOME/conspectus/` when
/// set, otherwise `$HOME/.local/share/conspectus/`, otherwise the
/// current directory — same lookup order as
/// [`crate::query::persist::graph_db_path`] so the daemon's two
/// artifacts (the legacy `graph.sqlite` and the new `graph.bin`)
/// live side-by-side during the P11 dual-write window.
pub fn graph_bin_path() -> PathBuf {
    let base = env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|h| PathBuf::from(h).join(".local").join("share")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("conspectus").join("graph.bin")
}

/// Errors returned by the snapshot reader/writer. Discriminated
/// so callers can pattern-match: a CLI fallback that wants to
/// distinguish "incompatible format, cold-rebuild" from
/// "transport-layer I/O error" reads
/// [`SnapshotError::IncompatibleVersion`] / [`SnapshotError::WrongMagic`]
/// as the cold-rebuild trigger and treats [`SnapshotError::Io`]
/// as a real failure.
#[derive(Debug, thiserror::Error)]
pub enum SnapshotError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("snapshot file truncated: needed {needed} bytes, found {actual}")]
    Truncated { needed: usize, actual: usize },

    #[error("snapshot file magic mismatch: expected {expected:?}, found {found:?}")]
    WrongMagic { expected: [u8; 8], found: [u8; 8] },

    #[error(
        "snapshot file format version {found} is incompatible with binary's expected {expected}"
    )]
    IncompatibleVersion { expected: u32, found: u32 },

    #[error("snapshot payload length {len} exceeds the {max} byte cap")]
    PayloadTooLarge { len: usize, max: usize },

    #[error("rkyv archive serialization failed: {0}")]
    Serialize(rancor::Error),

    #[error("rkyv archive validation failed: {0}")]
    Validate(rancor::Error),

    #[error("rkyv archive deserialization failed: {0}")]
    Deserialize(rancor::Error),
}

/// Result alias for the snapshot module.
pub type Result<T, E = SnapshotError> = std::result::Result<T, E>;

/// 32-byte file header per ADR 0083 §"File layout". Stable
/// across the lifetime of [`FORMAT_VERSION`] = 1; additive
/// header changes use the reserved bytes without bumping the
/// version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Header {
    pub magic: [u8; 8],
    pub format_version: u32,
    pub payload_len: u32,
    pub reserved: [u8; RESERVED_LEN],
}

impl Header {
    /// Produce a freshly-built header for a writer producing
    /// `payload_len` bytes. Reserved bytes are zeroed.
    pub fn new(payload_len: u32) -> Self {
        Self {
            magic: MAGIC,
            format_version: FORMAT_VERSION,
            payload_len,
            reserved: [0u8; RESERVED_LEN],
        }
    }

    /// Serialize to a fixed-width [u8; HEADER_LEN] buffer.
    pub fn to_bytes(&self) -> [u8; HEADER_LEN] {
        let mut buf = [0u8; HEADER_LEN];
        buf[0..8].copy_from_slice(&self.magic);
        buf[8..12].copy_from_slice(&self.format_version.to_le_bytes());
        buf[12..16].copy_from_slice(&self.payload_len.to_le_bytes());
        buf[16..32].copy_from_slice(&self.reserved);
        buf
    }

    /// Parse a header from a byte slice. Returns
    /// [`SnapshotError::Truncated`] if the slice is shorter than
    /// [`HEADER_LEN`], [`SnapshotError::WrongMagic`] when the
    /// magic does not match, and
    /// [`SnapshotError::IncompatibleVersion`] when the format
    /// version is anything other than [`FORMAT_VERSION`]. The
    /// asymmetric "any mismatch is incompatible" policy is
    /// deliberate per ADR 0082: there is no migration chain,
    /// and a version older or newer than the binary's expected
    /// version triggers cold rebuild equally.
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < HEADER_LEN {
            return Err(SnapshotError::Truncated {
                needed: HEADER_LEN,
                actual: bytes.len(),
            });
        }
        let mut magic = [0u8; 8];
        magic.copy_from_slice(&bytes[0..8]);
        if magic != MAGIC {
            return Err(SnapshotError::WrongMagic {
                expected: MAGIC,
                found: magic,
            });
        }
        let format_version = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
        if format_version != FORMAT_VERSION {
            return Err(SnapshotError::IncompatibleVersion {
                expected: FORMAT_VERSION,
                found: format_version,
            });
        }
        let payload_len = u32::from_le_bytes(bytes[12..16].try_into().unwrap());
        let mut reserved = [0u8; RESERVED_LEN];
        reserved.copy_from_slice(&bytes[16..32]);
        Ok(Self {
            magic,
            format_version,
            payload_len,
            reserved,
        })
    }
}

/// Serialize `snapshot` to the on-disk byte layout (header +
/// rkyv archive). Returns the bytes ready to be `mmap`'d or
/// written verbatim. The daemon dual-write path (P11-005) calls
/// this once per cycle so the same bytes can land in the
/// on-disk file *and* the in-memory cache the socket
/// `snapshot` command serves from.
pub fn serialize_to_bytes(snapshot: &GraphSnapshot) -> Result<Vec<u8>> {
    let payload = rkyv::to_bytes::<rancor::Error>(snapshot).map_err(SnapshotError::Serialize)?;
    let payload_len = u32::try_from(payload.len()).map_err(|_| SnapshotError::PayloadTooLarge {
        len: payload.len(),
        max: u32::MAX as usize,
    })?;
    let header = Header::new(payload_len);
    let mut buf = Vec::with_capacity(HEADER_LEN + payload.len());
    buf.extend_from_slice(&header.to_bytes());
    buf.extend_from_slice(&payload);
    Ok(buf)
}

/// Serialize `snapshot` and write the resulting header + payload
/// to `path` atomically. Convenience wrapper around
/// [`serialize_to_bytes`] + [`write_atomic_bytes`]. The bytes
/// land in `<path>.tmp.<pid>` first, are `fsync`ed, then renamed
/// over `path`. POSIX rename is atomic on the same filesystem;
/// the parent directory is `fsync`ed best-effort so the rename
/// itself is durable across a crash.
///
/// On failure mid-write, the tmp file may remain on disk; the
/// daemon's startup logic should clean stale tmp files matching
/// its glob.
pub fn write_atomic(path: &Path, snapshot: &GraphSnapshot) -> Result<()> {
    let bytes = serialize_to_bytes(snapshot)?;
    write_atomic_bytes(path, &bytes)
}

/// Atomic-rename write of pre-serialized snapshot bytes (header +
/// payload, as produced by [`serialize_to_bytes`]). Split out from
/// [`write_atomic`] so the daemon can serialize once for both the
/// on-disk file and the socket-cache.
pub fn write_atomic_bytes(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| std::io::Error::other("snapshot path has no parent directory"))?;
    if !parent.as_os_str().is_empty() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = tmp_path_for(path);

    {
        let mut tmp_file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&tmp)?;
        tmp_file.write_all(bytes)?;
        tmp_file.sync_all()?;
    }

    std::fs::rename(&tmp, path)?;

    // Best-effort parent-directory fsync so the rename itself is
    // durable across a crash. Failure is logged-and-ignored: the
    // file is already in place and a missed parent fsync only
    // matters if the machine power-cycles in the next moment.
    if !parent.as_os_str().is_empty()
        && let Ok(dir) = File::open(parent)
    {
        let _ = dir.sync_all();
    }

    Ok(())
}

/// Per-pid tmp path. The pid suffix lets two daemon-or-CLI
/// invocations on the same host write concurrently without
/// clobbering each other's tmp file — the rename arbitrates the
/// final state.
fn tmp_path_for(path: &Path) -> PathBuf {
    let pid = std::process::id();
    let mut name = path
        .file_name()
        .map(|os| os.to_string_lossy().into_owned())
        .unwrap_or_default();
    name.push_str(&format!(".tmp.{pid}"));
    let mut tmp = path.to_path_buf();
    tmp.set_file_name(name);
    tmp
}

/// Validated mmap handle. Holds the [`Mmap`] alive for the life
/// of the handle so the borrowed `&ArchivedGraphSnapshot`
/// returned by [`Self::archived`] stays valid as long as the
/// caller holds the handle. POSIX inode liveness guarantees the
/// mapped pages survive even if the on-disk file is renamed or
/// unlinked while the handle is alive.
#[derive(Debug)]
pub struct SnapshotMmap {
    mmap: Mmap,
    header: Header,
}

impl SnapshotMmap {
    /// Bytes range of the payload (the rkyv archive) within the
    /// mapped file. Excludes the header.
    pub fn payload(&self) -> &[u8] {
        let end = HEADER_LEN + self.header.payload_len as usize;
        &self.mmap[HEADER_LEN..end]
    }

    /// Borrow the archived snapshot directly out of the mapped
    /// pages. Validation already ran at [`open_mmap`] time, so
    /// this is `access_unchecked` — zero-cost.
    pub fn archived(&self) -> &ArchivedGraphSnapshot {
        // SAFETY: `open_mmap` ran `rkyv::access::<_, rancor::Error>`
        // against this same payload range, which validates the
        // archive end-to-end via `bytecheck`. The mapped pages
        // cannot mutate under us — POSIX inode liveness on Linux +
        // macOS keeps the original bytes addressable for the
        // lifetime of `self.mmap` regardless of whether the
        // on-disk file is replaced. Therefore the archive is
        // structurally valid and the borrow is sound.
        unsafe { rkyv::access_unchecked::<ArchivedGraphSnapshot>(self.payload()) }
    }

    /// The parsed header carried alongside the payload. Mostly
    /// useful for observability (logging the format version,
    /// inspecting reserved bytes) and for tests.
    pub fn header(&self) -> Header {
        self.header
    }
}

/// Open `path`, mmap it, parse + validate the header, then
/// validate the payload via `bytecheck`. The returned handle
/// derefs to `&ArchivedGraphSnapshot` via [`SnapshotMmap::archived`].
/// Validation policy follows ADR 0083 §"Validation": daemonless
/// CLI reads and daemon warm-start reads both validate; the
/// socket-served path uses [`open_mmap_unvalidated`] when it
/// trusts the source.
pub fn open_mmap(path: &Path) -> Result<SnapshotMmap> {
    let handle = open_mmap_unvalidated(path)?;
    rkyv::access::<ArchivedGraphSnapshot, rancor::Error>(handle.payload())
        .map_err(SnapshotError::Validate)?;
    Ok(handle)
}

/// Same as [`open_mmap`] but skips the `bytecheck` validation
/// pass. Use only when the bytes are known-good (e.g. served by
/// the daemon over the socket from its own `ArcSwap<Bytes>`
/// cache). Hostile or corrupted bytes are undefined behavior.
pub fn open_mmap_unvalidated(path: &Path) -> Result<SnapshotMmap> {
    let mut file = File::open(path)?;
    let file_len = file.metadata()?.len();
    if file_len < HEADER_LEN as u64 {
        return Err(SnapshotError::Truncated {
            needed: HEADER_LEN,
            actual: file_len as usize,
        });
    }

    let mut header_buf = [0u8; HEADER_LEN];
    file.seek(SeekFrom::Start(0))?;
    file.read_exact(&mut header_buf)?;
    let header = Header::parse(&header_buf)?;

    let payload_len = header.payload_len as usize;
    if payload_len > MAX_PAYLOAD_LEN {
        return Err(SnapshotError::PayloadTooLarge {
            len: payload_len,
            max: MAX_PAYLOAD_LEN,
        });
    }
    let expected_total = HEADER_LEN + payload_len;
    if (file_len as usize) < expected_total {
        return Err(SnapshotError::Truncated {
            needed: expected_total,
            actual: file_len as usize,
        });
    }

    // SAFETY: `Mmap::map` is unsafe because the caller asserts
    // no other process will mutate the file under us. Conspectus
    // is single-user single-machine (ADR 0036); the daemon's
    // atomic-rename writer never mutates an existing file in
    // place — it creates a new file and renames over the path,
    // and POSIX inode liveness keeps our mapped pages pointing
    // at the original inode for the life of the `Mmap`. The
    // worst-case is a hostile other-user process truncating the
    // file mid-read, which Conspectus's single-user threat model
    // (ADR 0038's 0600 permissions on the runtime dir) does not
    // promise to defend against.
    let mmap = unsafe { Mmap::map(&file)? };

    Ok(SnapshotMmap { mmap, header })
}

/// Explicit escape hatch for callers that want an owned
/// [`GraphSnapshot`] — typically tests, the JSON-dump path, or
/// any consumer that intends to mutate the result. The
/// conversion walks the archive once and allocates a fresh
/// owned tree. Daemon-resident readers prefer
/// [`SnapshotMmap::archived`] to skip the allocation.
pub fn deserialize_owned(handle: &SnapshotMmap) -> Result<GraphSnapshot> {
    rkyv::deserialize::<GraphSnapshot, rancor::Error>(handle.archived())
        .map_err(SnapshotError::Deserialize)
}

#[cfg(test)]
mod tests {
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
}
