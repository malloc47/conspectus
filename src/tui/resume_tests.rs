use super::*;

#[test]
fn launch_target_carries_the_session_cwd() {
    let session = AgentSessionId::new("claude-code", "/state", "abc");
    match resolve_resume_target(&session, Some(Path::new("/work/project"))) {
        ResumeTarget::Launch { cwd, .. } => assert_eq!(cwd, Path::new("/work/project")),
        other => panic!("expected Launch, got {other:?}"),
    }
}

#[test]
fn supported_harness_without_a_cwd_is_not_resumable() {
    let session = AgentSessionId::new("claude-code", "/state", "abc");
    assert_eq!(resolve_resume_target(&session, None), ResumeTarget::NoCwd);
}

#[test]
fn unsupported_harness_reports_unsupported_even_with_a_cwd() {
    let session = AgentSessionId::new("aider", "/state", "abc");
    assert!(matches!(
        resolve_resume_target(&session, Some(Path::new("/work"))),
        ResumeTarget::Unsupported { .. }
    ));
}
