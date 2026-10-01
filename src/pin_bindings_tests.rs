use super::*;
use std::path::Path;
use tempfile::tempdir;

fn sample(observed_epoch: i64) -> PinBindingRecord {
    PinBindingRecord::new("ingest", "ingest", None, "abc123", "codex", observed_epoch)
}

fn cache_in(dir: &Path) -> PinBindingsCache {
    PinBindingsCache::new().with_xdg_cache_home(dir)
}

#[test]
fn round_trip_preserves_every_field() {
    let record = PinBindingRecord {
        schema_version: PIN_BINDING_SCHEMA_VERSION,
        pin_id: "ingest".into(),
        mux_name: "ingest-refactor".into(),
        mux_socket: Some("scratch".into()),
        session_id: "abc123".into(),
        harness: "codex".into(),
        observed_epoch: 1_738_742_400,
    };
    let json = to_json(&record).unwrap();
    let parsed = parse_record(&json).unwrap();
    assert_eq!(parsed, record);
}

#[test]
fn unknown_json_keys_are_tolerated_on_read() {
    let payload = r#"{
            "schema_version": 1,
            "pin_id": "ingest",
            "mux_name": "ingest",
            "session_id": "abc",
            "harness": "codex",
            "observed_epoch": 1738742400,
            "future_field": "ignored"
        }"#;
    let parsed = parse_record(payload).expect("unknown key tolerance");
    assert_eq!(parsed.pin_id, "ingest");
}

#[test]
fn malformed_json_surfaces_diagnostic() {
    let err = parse_record("not json").unwrap_err();
    assert!(matches!(err, PinBindingError::MalformedJson(_)));
}

#[test]
fn unsupported_schema_version_rejected() {
    let payload = r#"{
            "schema_version": 99,
            "pin_id": "ingest",
            "mux_name": "ingest",
            "session_id": "abc",
            "harness": "codex",
            "observed_epoch": 1
        }"#;
    let err = parse_record(payload).unwrap_err();
    assert!(matches!(err, PinBindingError::UnsupportedSchemaVersion(99)));
}

#[test]
fn empty_required_fields_rejected() {
    let mut record = sample(1);
    record.session_id = String::new();
    let err = validate(&record).unwrap_err();
    assert!(matches!(
        err,
        PinBindingError::EmptyField {
            field: "session_id",
            ..
        }
    ));
}

#[test]
fn whitespace_socket_rejected_as_empty() {
    let mut record = sample(1);
    record.mux_socket = Some("   ".into());
    let err = validate(&record).unwrap_err();
    assert!(matches!(
        err,
        PinBindingError::EmptyField {
            field: "mux_socket",
            ..
        }
    ));
}

#[test]
fn directory_uses_xdg_cache_when_set() {
    let temp = tempdir().unwrap();
    let cache = cache_in(temp.path());
    let dir = cache.directory().unwrap();
    assert_eq!(dir, temp.path().join("conspectus").join("pin-bindings"));
}

#[test]
fn directory_falls_back_to_home_dot_cache() {
    let temp = tempdir().unwrap();
    let cache = PinBindingsCache::new().with_home(temp.path());
    let dir = cache.directory().unwrap();
    assert_eq!(
        dir,
        temp.path()
            .join(".cache")
            .join("conspectus")
            .join("pin-bindings"),
    );
}

#[test]
fn directory_none_without_env_overrides() {
    let cache = PinBindingsCache::new();
    assert!(cache.directory().is_none());
}

#[test]
fn path_for_resolves_per_pin_filename() {
    let temp = tempdir().unwrap();
    let cache = cache_in(temp.path());
    let path = cache.path_for("ingest").unwrap();
    assert!(path.ends_with("conspectus/pin-bindings/ingest.json"));
}

#[test]
fn read_returns_none_when_directory_absent() {
    let temp = tempdir().unwrap();
    let cache = cache_in(temp.path());
    let result = read(&cache, "ingest").unwrap();
    assert!(result.is_none());
}

#[test]
fn write_then_read_round_trips_through_disk() {
    let temp = tempdir().unwrap();
    let cache = cache_in(temp.path());
    let record = sample(1_738_742_400);
    let outcome = write(&cache, &record).unwrap();
    assert_eq!(outcome, WriteOutcome::Wrote);

    let read_back = read(&cache, "ingest").unwrap().expect("sidecar exists");
    assert_eq!(read_back, record);
}

#[test]
fn write_creates_parent_directories_on_first_use() {
    let temp = tempdir().unwrap();
    let cache = cache_in(temp.path());
    // Directory does not yet exist.
    let dir = cache.directory().unwrap();
    assert!(!dir.exists());

    write(&cache, &sample(1)).unwrap();

    assert!(dir.is_dir());
    assert!(dir.join("ingest.json").is_file());
}

#[test]
fn write_skips_when_payload_unchanged() {
    let temp = tempdir().unwrap();
    let cache = cache_in(temp.path());
    let record = sample(1);

    let first = write(&cache, &record).unwrap();
    assert_eq!(first, WriteOutcome::Wrote);

    // Second write with the same record should skip — no mtime
    // churn on quiet cycles per ADR 0058 §Write path.
    let second = write(&cache, &record).unwrap();
    assert_eq!(second, WriteOutcome::Skipped);
}

#[test]
fn write_replaces_when_payload_changes() {
    let temp = tempdir().unwrap();
    let cache = cache_in(temp.path());

    write(&cache, &sample(1)).unwrap();
    let second = write(&cache, &sample(2)).unwrap();
    assert_eq!(second, WriteOutcome::Wrote);

    let read_back = read(&cache, "ingest").unwrap().unwrap();
    assert_eq!(read_back.observed_epoch, 2);
}

#[test]
fn write_refuses_invalid_records() {
    let temp = tempdir().unwrap();
    let cache = cache_in(temp.path());
    let mut record = sample(1);
    record.harness = String::new();
    let err = write(&cache, &record).unwrap_err();
    assert!(matches!(
        err,
        PinBindingError::EmptyField {
            field: "harness",
            ..
        }
    ));
    // Nothing was written to disk.
    assert!(read(&cache, "ingest").unwrap().is_none());
}

#[test]
fn write_without_cache_root_errors() {
    let cache = PinBindingsCache::new();
    let err = write(&cache, &sample(1)).unwrap_err();
    assert!(matches!(err, PinBindingError::Io(_)));
}

#[test]
fn read_surfaces_malformed_file_without_writing() {
    let temp = tempdir().unwrap();
    let cache = cache_in(temp.path());
    let dir = cache.directory().unwrap();
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("ingest.json"), b"this is not json").unwrap();

    let err = read(&cache, "ingest").unwrap_err();
    assert!(matches!(err, PinBindingError::MalformedJson(_)));
}

#[test]
fn delete_removes_existing_sidecar() {
    let temp = tempdir().unwrap();
    let cache = cache_in(temp.path());
    write(&cache, &sample(1)).unwrap();

    let removed = delete(&cache, "ingest").unwrap();
    assert!(removed);
    assert!(read(&cache, "ingest").unwrap().is_none());
}

#[test]
fn delete_returns_false_when_sidecar_missing() {
    let temp = tempdir().unwrap();
    let cache = cache_in(temp.path());
    let removed = delete(&cache, "ingest").unwrap();
    assert!(!removed);
}

#[test]
fn delete_is_safe_without_cache_root() {
    let cache = PinBindingsCache::new();
    let removed = delete(&cache, "ingest").unwrap();
    assert!(!removed);
}

// ----- record_bindings: post-resolve write pass -----

mod record {
    use super::*;
    use crate::model::{
        AgentSessionId, GraphSnapshot, MuxSessionId, PinBinding, PinCandidate, PinMuxRef,
        Provenance,
    };

    fn make_pin(id: &str, binding: Option<PinBinding>) -> PinCandidate {
        PinCandidate {
            id: id.to_string(),
            display_name: id.to_string(),
            harness: "codex".to_string(),
            cwd: "/p".to_string(),
            mux: PinMuxRef {
                backend: "tmux".to_string(),
                name: id.to_string(),
                socket_name: None,
            },
            launch_argv: None,
            reason: None,
            provenance: Provenance::LocalPin,
            store_path: "/p/.conspectus.toml".to_string(),
            binding,
        }
    }

    fn bound(session_key: &str, mux_name: &str) -> PinBinding {
        PinBinding::Bound {
            mux: MuxSessionId::new(format!("tmux:{mux_name}")),
            session: AgentSessionId::new("codex", "/state", session_key),
        }
    }

    #[test]
    fn writes_sidecar_for_each_bound_pin() {
        let temp = tempdir().unwrap();
        let cache = cache_in(temp.path());
        let mut snap = GraphSnapshot::empty();
        snap.pins
            .push(make_pin("ingest", Some(bound("session-a", "ingest"))));
        snap.pins
            .push(make_pin("review", Some(bound("session-b", "review"))));

        let outcomes = record_bindings(&snap, &cache, 1_738_742_400);

        assert_eq!(outcomes.len(), 2);
        assert!(outcomes.iter().all(|(_, r)| r.is_ok()));

        let ingest = read(&cache, "ingest").unwrap().expect("ingest sidecar");
        assert_eq!(ingest.session_id, "session-a");
        assert_eq!(ingest.observed_epoch, 1_738_742_400);

        let review = read(&cache, "review").unwrap().expect("review sidecar");
        assert_eq!(review.session_id, "session-b");
    }

    #[test]
    fn skips_unbound_and_stale_pins() {
        let temp = tempdir().unwrap();
        let cache = cache_in(temp.path());
        let mut snap = GraphSnapshot::empty();
        snap.pins.push(make_pin("u", Some(PinBinding::Unbound)));
        snap.pins.push(make_pin(
            "s",
            Some(PinBinding::StaleMux {
                mux: MuxSessionId::new("tmux:s"),
            }),
        ));
        snap.pins.push(make_pin("n", None));

        let outcomes = record_bindings(&snap, &cache, 1);

        assert!(
            outcomes.is_empty(),
            "no Bound pins should produce no writes"
        );
        assert!(read(&cache, "u").unwrap().is_none());
        assert!(read(&cache, "s").unwrap().is_none());
        assert!(read(&cache, "n").unwrap().is_none());
    }

    #[test]
    fn empty_pins_is_noop() {
        let temp = tempdir().unwrap();
        let cache = cache_in(temp.path());
        let snap = GraphSnapshot::empty();
        let outcomes = record_bindings(&snap, &cache, 1);
        assert!(outcomes.is_empty());
    }

    #[test]
    fn idempotent_second_call_skips_unchanged() {
        let temp = tempdir().unwrap();
        let cache = cache_in(temp.path());
        let mut snap = GraphSnapshot::empty();
        snap.pins
            .push(make_pin("ingest", Some(bound("session-a", "ingest"))));

        let first = record_bindings(&snap, &cache, 1);
        let second = record_bindings(&snap, &cache, 1);

        assert_eq!(first[0].1.as_ref().unwrap(), &WriteOutcome::Wrote);
        assert_eq!(second[0].1.as_ref().unwrap(), &WriteOutcome::Skipped);
    }

    #[test]
    fn observed_epoch_change_triggers_replacement() {
        let temp = tempdir().unwrap();
        let cache = cache_in(temp.path());
        let mut snap = GraphSnapshot::empty();
        snap.pins
            .push(make_pin("ingest", Some(bound("session-a", "ingest"))));

        let _ = record_bindings(&snap, &cache, 1);
        let second = record_bindings(&snap, &cache, 2);

        assert_eq!(second[0].1.as_ref().unwrap(), &WriteOutcome::Wrote);
        let stored = read(&cache, "ingest").unwrap().unwrap();
        assert_eq!(stored.observed_epoch, 2);
    }

    #[test]
    fn carries_mux_socket_into_sidecar() {
        let temp = tempdir().unwrap();
        let cache = cache_in(temp.path());
        let mut pin = make_pin("ingest", Some(bound("session-a", "ingest")));
        pin.mux.socket_name = Some("scratch".to_string());
        let mut snap = GraphSnapshot::empty();
        snap.pins.push(pin);

        record_bindings(&snap, &cache, 1);

        let stored = read(&cache, "ingest").unwrap().unwrap();
        assert_eq!(stored.mux_socket.as_deref(), Some("scratch"));
    }

    #[test]
    fn cache_without_root_surfaces_error_per_pin() {
        let cache = PinBindingsCache::new();
        let mut snap = GraphSnapshot::empty();
        snap.pins
            .push(make_pin("ingest", Some(bound("session-a", "ingest"))));

        let outcomes = record_bindings(&snap, &cache, 1);

        assert_eq!(outcomes.len(), 1);
        assert!(outcomes[0].1.is_err());
    }
}

// ----- lineage_head: forward walk through parent_session links -----

mod lineage {
    use super::*;
    use crate::model::{
        AgentSessionId, AgentSessionNode, Confidence, Diagnostic, Freshness, GraphLink, GraphNode,
        GraphSnapshot, LinkEndpoint, LinkState, NodeId, PinLastSession, Provenance, RelationKind,
        SourceMetadata,
    };

    fn session(key: &str) -> AgentSessionNode {
        AgentSessionNode {
            id: AgentSessionId::new("codex", "/state", key),
            harness_key: "codex".to_string(),
            cwd: None,
            title: None,
            last_message_preview: None,
            last_active_epoch: None,
            session_kind: None,
        }
    }

    /// Append a `child --parent_session--> parent` link to `snap`.
    fn parent_link(snap: &mut GraphSnapshot, child: &str, parent: &str) {
        snap.candidate_links.push(GraphLink {
            id: format!("{child}-parent-of-{parent}"),
            source: NodeId::AgentSession(AgentSessionId::new("codex", "/state", child)),
            target: LinkEndpoint::Node {
                id: NodeId::AgentSession(AgentSessionId::new("codex", "/state", parent)),
            },
            relation: RelationKind::ParentSession,
            provenance: Provenance::StrongDiscovered,
            confidence: Confidence::High,
            freshness: Freshness::Fresh,
            source_metadata: SourceMetadata::default(),
            state: LinkState::Active,
        });
    }

    #[test]
    fn session_missing_when_not_in_snapshot() {
        let snap = GraphSnapshot::empty();
        assert_eq!(
            lineage_head(&snap, "codex", "nowhere"),
            LineageOutcome::SessionMissing,
        );
    }

    #[test]
    fn no_successors_returns_current_as_head() {
        let mut snap = GraphSnapshot::empty();
        snap.nodes.push(GraphNode::AgentSession(session("a")));

        match lineage_head(&snap, "codex", "a") {
            LineageOutcome::Head(id) => assert_eq!(id.session_key, "a"),
            other => panic!("expected Head, got {other:?}"),
        }
    }

    #[test]
    fn walks_single_successor_chain_to_leaf() {
        // a -> b -> c (each child's parent_session points back)
        let mut snap = GraphSnapshot::empty();
        for key in ["a", "b", "c"] {
            snap.nodes.push(GraphNode::AgentSession(session(key)));
        }
        parent_link(&mut snap, "b", "a");
        parent_link(&mut snap, "c", "b");

        match lineage_head(&snap, "codex", "a") {
            LineageOutcome::Head(id) => assert_eq!(id.session_key, "c"),
            other => panic!("expected Head=c, got {other:?}"),
        }
    }

    #[test]
    fn fork_stops_walk_and_lists_successors() {
        // a -> {b, c}  (both b and c have parent_session = a)
        let mut snap = GraphSnapshot::empty();
        for key in ["a", "b", "c"] {
            snap.nodes.push(GraphNode::AgentSession(session(key)));
        }
        parent_link(&mut snap, "b", "a");
        parent_link(&mut snap, "c", "a");

        match lineage_head(&snap, "codex", "a") {
            LineageOutcome::Fork { at, successors } => {
                assert_eq!(at.session_key, "a");
                let keys: Vec<&str> = successors.iter().map(|s| s.session_key.as_str()).collect();
                assert_eq!(keys, vec!["b", "c"]);
            }
            other => panic!("expected Fork at a, got {other:?}"),
        }
    }

    #[test]
    fn ignores_inactive_parent_links() {
        // a -> b with state=Ignored should be skipped.
        let mut snap = GraphSnapshot::empty();
        for key in ["a", "b"] {
            snap.nodes.push(GraphNode::AgentSession(session(key)));
        }
        parent_link(&mut snap, "b", "a");
        snap.candidate_links[0].state = LinkState::Ignored { reason: None };

        match lineage_head(&snap, "codex", "a") {
            LineageOutcome::Head(id) => assert_eq!(id.session_key, "a"),
            other => panic!("expected Head=a (b's link is inactive), got {other:?}"),
        }
    }

    #[test]
    fn cycle_in_parent_session_returns_best_effort_head() {
        // a -> b -> a — malformed but the walk must not loop.
        let mut snap = GraphSnapshot::empty();
        for key in ["a", "b"] {
            snap.nodes.push(GraphNode::AgentSession(session(key)));
        }
        parent_link(&mut snap, "b", "a");
        parent_link(&mut snap, "a", "b");

        // Either a or b is acceptable depending on walk order;
        // both are reachable and the visited-set kicks in on the
        // second step. The key invariant is no infinite loop.
        match lineage_head(&snap, "codex", "a") {
            LineageOutcome::Head(_) => {}
            other => panic!("expected Head (cycle defense), got {other:?}"),
        }
    }

    // ----- decorate_unbound_diagnostics: post-resolve enrichment -----

    #[test]
    fn decorate_populates_last_session_from_sidecar() {
        let temp = tempdir().unwrap();
        let cache = PinBindingsCache::new().with_xdg_cache_home(temp.path());
        // Seed a sidecar for `ingest`.
        let record = PinBindingRecord::new(
            "ingest",
            "ingest",
            None,
            "session-a",
            "codex",
            1_738_742_400,
        );
        write(&cache, &record).unwrap();

        let mut snap = GraphSnapshot::empty();
        snap.diagnostics.push(Diagnostic::PinUnbound {
            pin_id: "ingest".to_string(),
            expected_mux_native_id: "tmux:ingest".to_string(),
            last_session: None,
        });

        decorate_unbound_diagnostics(&mut snap, &cache);

        match &snap.diagnostics[0] {
            Diagnostic::PinUnbound { last_session, .. } => {
                let last = last_session
                    .as_ref()
                    .expect("decorator populated last_session");
                assert_eq!(last.session_id, "session-a");
                assert_eq!(last.observed_epoch, 1_738_742_400);
            }
            other => panic!("expected PinUnbound, got {other:?}"),
        }
    }

    #[test]
    fn decorate_leaves_unbound_alone_when_no_sidecar() {
        let temp = tempdir().unwrap();
        let cache = PinBindingsCache::new().with_xdg_cache_home(temp.path());

        let mut snap = GraphSnapshot::empty();
        snap.diagnostics.push(Diagnostic::PinUnbound {
            pin_id: "noprior".to_string(),
            expected_mux_native_id: "tmux:noprior".to_string(),
            last_session: None,
        });

        decorate_unbound_diagnostics(&mut snap, &cache);

        match &snap.diagnostics[0] {
            Diagnostic::PinUnbound { last_session, .. } => assert!(last_session.is_none()),
            other => panic!("expected PinUnbound, got {other:?}"),
        }
    }

    #[test]
    fn decorate_skips_already_populated_diagnostics() {
        let temp = tempdir().unwrap();
        let cache = PinBindingsCache::new().with_xdg_cache_home(temp.path());
        // Sidecar disagrees with the pre-populated diagnostic;
        // we want the existing value preserved.
        let record = PinBindingRecord::new("ingest", "ingest", None, "from-sidecar", "codex", 2);
        write(&cache, &record).unwrap();

        let mut snap = GraphSnapshot::empty();
        snap.diagnostics.push(Diagnostic::PinUnbound {
            pin_id: "ingest".to_string(),
            expected_mux_native_id: "tmux:ingest".to_string(),
            last_session: Some(PinLastSession {
                session_id: "pre-populated".to_string(),
                observed_epoch: 1,
            }),
        });

        decorate_unbound_diagnostics(&mut snap, &cache);

        match &snap.diagnostics[0] {
            Diagnostic::PinUnbound { last_session, .. } => {
                let last = last_session.as_ref().unwrap();
                assert_eq!(last.session_id, "pre-populated");
            }
            other => panic!("expected PinUnbound, got {other:?}"),
        }
    }

    #[test]
    fn decorate_ignores_non_pinunbound_diagnostics() {
        let temp = tempdir().unwrap();
        let cache = PinBindingsCache::new().with_xdg_cache_home(temp.path());

        let mut snap = GraphSnapshot::empty();
        snap.diagnostics.push(Diagnostic::PinDrift {
            pin_id: "ingest".to_string(),
            declared_cwd: "/p".to_string(),
            observed_cwd: "/q".to_string(),
        });

        decorate_unbound_diagnostics(&mut snap, &cache);

        // Untouched.
        assert!(matches!(&snap.diagnostics[0], Diagnostic::PinDrift { .. }));
    }

    #[test]
    fn other_harness_links_are_invisible() {
        // a (codex) -> b (codex) succession exists; an unrelated
        // claude-code session that happens to share the key "a"
        // shouldn't interfere.
        let mut snap = GraphSnapshot::empty();
        snap.nodes.push(GraphNode::AgentSession(session("a")));
        snap.nodes.push(GraphNode::AgentSession(session("b")));
        snap.nodes.push(GraphNode::AgentSession(AgentSessionNode {
            id: AgentSessionId::new("claude-code", "/cc", "a"),
            harness_key: "claude-code".to_string(),
            cwd: None,
            title: None,
            last_message_preview: None,
            last_active_epoch: None,
            session_kind: None,
        }));
        parent_link(&mut snap, "b", "a");

        match lineage_head(&snap, "codex", "a") {
            LineageOutcome::Head(id) => {
                assert_eq!(id.harness_key, "codex");
                assert_eq!(id.session_key, "b");
            }
            other => panic!("expected Head=codex:b, got {other:?}"),
        }
    }
}
