use super::*;

#[test]
fn ttl_cache_returns_value_for_matching_key_until_ttl() {
    let cache: TtlCache<u8, &str> = TtlCache::new(Duration::from_secs(60));
    assert_eq!(cache.get(&1), None);
    cache.set(1, "a");
    assert_eq!(cache.get(&1), Some("a"));
    assert_eq!(cache.get(&2), None, "a different key misses");

    let expired: TtlCache<(), &str> = TtlCache::new(Duration::ZERO);
    expired.set((), "a");
    assert_eq!(expired.get(&()), None, "a zero TTL never hits");
}

#[test]
fn stamped_map_hits_only_while_the_stamp_matches() {
    let map: StampedMap<u32, &str> = StampedMap::new();
    let path = Path::new("/x");
    assert_eq!(map.get(path, &1), None);
    map.insert(path.to_path_buf(), 1, "v1");
    assert_eq!(map.get(path, &1), Some("v1"));
    assert_eq!(map.get(path, &2), None);
    assert_eq!(
        map.get_if(path, |stamp, value| *stamp == 1 && *value == "v1"),
        Some("v1")
    );
}

#[test]
fn file_stamp_changes_when_the_file_changes() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("f");
    std::fs::write(&path, "one").expect("write");
    let first = FileStamp::of(&path).expect("stamp");
    assert_eq!(FileStamp::of(&path), Some(first));
    std::fs::write(&path, "longer").expect("write");
    assert_ne!(FileStamp::of(&path), Some(first), "length changed");
    assert!(FileStamp::of(&dir.path().join("missing")).is_none());
}
