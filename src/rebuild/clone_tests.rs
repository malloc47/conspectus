use super::*;
use crate::discovery::tmux::FakeTmux;
use crate::model::GraphNode;
use crate::model::{
    FieldValue, HistorySnapshot, NodePresence, RecordMeta, RecordedRemote, RepoRecord,
};
use crate::rebuild::resolve_subject;
use crate::rebuild::test_support::{Harness, titles};

fn repo_snapshot(dir: &Path, remote: Option<&str>) -> GraphSnapshot {
    let mut history = HistorySnapshot {
        repos: vec![RepoRecord {
            meta: RecordMeta {
                id: "h_repo".into(),
                key: String::new(),
                first_seen: 1,
                last_seen: 100,
                origin: Default::default(),
                display: Default::default(),
                rebuilt_from: None,
            },
            recipe: crate::model::RepoRecipe {
                common_dir: dir.join(".git").display().to_string(),
                remotes: remote
                    .map(|url| {
                        vec![RecordedRemote {
                            name: "origin".into(),
                            url: url.into(),
                        }]
                    })
                    .unwrap_or_default(),
                primary_root: Some(dir.display().to_string()),
                default_branch: None,
                extra: Default::default(),
            },
            extra: Default::default(),
        }],
        ..Default::default()
    };
    history
        .presence
        .insert("h_repo".into(), NodePresence::Gone { last_seen: 100 });
    GraphSnapshot {
        history,
        ..GraphSnapshot::empty()
    }
}

fn plan_for(snapshot: &GraphSnapshot, h: &Harness) -> Result<Plan, RebuildError> {
    let subject = resolve_subject(snapshot, &[], "h_repo").unwrap();
    h.plan(snapshot, &subject, &Options::default())
}

#[test]
fn a_gone_repo_is_cloned_from_its_first_recorded_remote_to_its_path() {
    let tmp = tempfile::tempdir().unwrap();
    let dest = tmp.path().join("old");
    let snapshot = repo_snapshot(&dest, Some("git@github.com:me/old.git"));
    let h = Harness::new(FakeTmux::with_sessions(""));
    let plan = plan_for(&snapshot, &h).unwrap();
    assert_eq!(plan.subject, "repo `old` (removed)");
    assert_eq!(titles(&plan), ["clone the repo"]);
    assert_eq!(
        plan.steps[0].command,
        format!("git clone git@github.com:me/old.git {}", dest.display())
    );
    h.run(&plan).unwrap();
    assert_eq!(
        h.cloner.calls.lock().unwrap()[0].0,
        "git@github.com:me/old.git"
    );
}

#[test]
fn a_slot_can_override_the_remote_and_the_path() {
    let tmp = tempfile::tempdir().unwrap();
    let snapshot = repo_snapshot(&tmp.path().join("old"), Some("git@github.com:me/old.git"));
    let mut subject = resolve_subject(&snapshot, &[], "h_repo").unwrap();
    subject.slot.overrides.insert(
        "remote".into(),
        FieldValue::Text("https://example.com/mirror.git".into()),
    );
    subject.slot.overrides.insert(
        "path".into(),
        FieldValue::Text(tmp.path().join("new-home").display().to_string()),
    );
    let h = Harness::new(FakeTmux::with_sessions(""));
    let plan = h.plan(&snapshot, &subject, &Options::default()).unwrap();
    h.run(&plan).unwrap();
    let (remote, dest) = h.cloner.calls.lock().unwrap()[0].clone();
    assert_eq!(remote, "https://example.com/mirror.git");
    assert_eq!(dest, tmp.path().join("new-home"));
}

#[test]
fn a_clone_never_goes_over_an_existing_path() {
    let tmp = tempfile::tempdir().unwrap();
    let snapshot = repo_snapshot(tmp.path(), Some("git@github.com:me/old.git"));
    let h = Harness::new(FakeTmux::with_sessions(""));
    let err = plan_for(&snapshot, &h).unwrap_err();
    assert!(err.to_string().contains("already exists"), "{err}");
    assert!(h.cloner.calls.lock().unwrap().is_empty());
}

#[test]
fn a_repo_with_no_remote_names_the_fix() {
    let tmp = tempfile::tempdir().unwrap();
    let snapshot = repo_snapshot(&tmp.path().join("old"), None);
    let h = Harness::new(FakeTmux::with_sessions(""));
    let err = plan_for(&snapshot, &h).unwrap_err();
    assert!(err.to_string().contains("--set remote="), "{err}");
}

#[test]
fn clone_arguments_are_checked() {
    let tmp = tempfile::tempdir().unwrap();
    let dest = tmp.path().join("fresh");
    assert!(check_clone_args("git@github.com:me/x.git", &dest).is_ok());
    assert!(check_clone_args("--upload-pack=evil", &dest).is_err());
    assert!(check_clone_args("", &dest).is_err());
    assert!(check_clone_args("git@github.com:me/x.git", Path::new("relative")).is_err());
    assert!(check_clone_args("git@github.com:me/x.git", tmp.path()).is_err());
}

#[test]
fn the_system_cloner_really_clones_a_local_repo_and_refuses_an_existing_path() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("source");
    std::fs::create_dir_all(&source).unwrap();
    for args in [
        &["init", "--quiet", "--initial-branch", "main"][..],
        &[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "commit",
            "--quiet",
            "--allow-empty",
            "-m",
            "i",
        ][..],
    ] {
        assert!(
            std::process::Command::new("git")
                .args(args)
                .current_dir(&source)
                .status()
                .unwrap()
                .success()
        );
    }
    let dest = tmp.path().join("copy");
    SystemCloner
        .clone_repo(source.to_str().unwrap(), &dest)
        .unwrap();
    assert!(dest.join(".git").is_dir());
    assert!(
        SystemCloner
            .clone_repo(source.to_str().unwrap(), &dest)
            .is_err()
    );
}

#[test]
fn a_live_repo_has_nothing_to_clone() {
    let tmp = tempfile::tempdir().unwrap();
    let mut snapshot = repo_snapshot(tmp.path(), Some("git@github.com:me/old.git"));
    snapshot.history.presence.clear();
    snapshot
        .nodes
        .push(GraphNode::Repo(crate::model::RepoNode::new(
            crate::model::RepoId::new(tmp.path().join(".git").display().to_string()),
        )));
    let h = Harness::new(FakeTmux::with_sessions(""));
    let plan = plan_for(&snapshot, &h).unwrap();
    assert!(plan.steps.is_empty());
}
