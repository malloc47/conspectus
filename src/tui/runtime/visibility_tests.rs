use super::*;

/// What a poll said: `clients:window:pane:last session`.
fn polled(text: &str) -> Option<Observation> {
    parse_observation(text)
}

fn watch() -> Watch {
    Watch::new(Some("/tmp/tmux-1000/default".to_string()))
}

fn came_back(session: &str, at_ms: i64) -> Signal {
    Signal::Returned(ClientReturn {
        socket: "/tmp/tmux-1000/default".to_string(),
        session: session.to_string(),
        at_ms,
    })
}

#[test]
fn a_pane_is_visible_only_when_attached_and_active() {
    let visible = |text: &str| polled(text).map(|seen| seen.visible());
    assert_eq!(visible("1:1:1\n"), Some(true));
    assert_eq!(visible("2:1:1"), Some(true));
    assert_eq!(visible("0:1:1"), Some(false), "no client on the session");
    assert_eq!(visible("1:0:1"), Some(false), "another window is showing");
    assert_eq!(visible("1:1:0"), Some(false), "another pane is active");
}

#[test]
fn the_clients_last_session_is_the_fourth_field_when_it_has_one() {
    let last = |text: &str| polled(text).and_then(|seen| seen.last_session);
    assert_eq!(last("1:1:1:mux\n"), Some("mux".to_string()));
    assert_eq!(last("1:1:1:"), None, "the client has not switched yet");
    assert_eq!(last("1:1:1"), None, "an older tmux prints no fourth field");
    assert_eq!(
        last("1:1:1:a:b"),
        Some("a:b".to_string()),
        "everything after the third colon is the name"
    );
}

#[test]
fn unreadable_output_says_nothing() {
    assert_eq!(polled(""), None);
    assert_eq!(polled("garbage"), None);
    assert_eq!(polled("1:1"), None);
}

#[test]
fn only_a_hidden_to_visible_change_regains() {
    let mut watch = watch();
    let mut poll = |text: &str, at_ms: i64| watch.step(polled(text), at_ms);
    assert_eq!(poll("1:1:1:", 1_000), vec![], "staying in view");
    assert_eq!(poll("1:0:1:", 1_250), vec![], "leaving view");
    assert_eq!(poll("1:0:1:", 1_500), vec![], "still away");
    assert_eq!(
        poll("1:1:1:", 1_750),
        vec![Signal::Regained],
        "back in view"
    );
    assert_eq!(poll("1:1:1:", 2_000), vec![], "and in view");
}

#[test]
fn a_failed_poll_neither_fires_nor_changes_state() {
    let mut watch = watch();
    assert_eq!(watch.step(polled("0:1:1:"), 1_000), vec![]);
    assert_eq!(watch.step(None, 1_250), vec![]);
    assert_eq!(
        watch.step(polled("1:1:1:mux"), 1_500),
        vec![came_back("mux", 1_000), Signal::Regained],
        "the poll that failed was not an observation of the client away"
    );
}

#[test]
fn the_client_coming_back_from_a_session_names_it_and_the_last_poll_it_was_away() {
    let mut watch = watch();
    assert_eq!(watch.step(polled("1:1:1:"), 1_000), vec![]);
    assert_eq!(
        watch.step(polled("0:1:1:"), 1_250),
        vec![],
        "the client left"
    );
    assert_eq!(watch.step(polled("0:1:1:"), 1_500), vec![]);
    assert_eq!(
        watch.step(polled("1:1:1:mux"), 1_750),
        vec![came_back("mux", 1_500), Signal::Regained],
        "it came back from `mux`; nothing is credited after the last poll it was away"
    );
    assert_eq!(
        watch.step(polled("1:1:1:mux"), 2_000),
        vec![],
        "one return fires once"
    );
}

#[test]
fn a_window_change_inside_the_tuis_session_credits_no_session() {
    // The client's last session is where it was before it came to the
    // TUI's session, however long ago, and says nothing of a window
    // change.
    let mut watch = watch();
    assert_eq!(watch.step(polled("1:1:1:mux"), 1_000), vec![]);
    assert_eq!(watch.step(polled("1:0:1:mux"), 1_250), vec![]);
    assert_eq!(
        watch.step(polled("1:1:1:mux"), 1_500),
        vec![Signal::Regained],
        "back to the TUI's window, from another window of the same session"
    );
}

#[test]
fn a_return_to_another_window_is_credited_once_and_then_the_regain_comes_alone() {
    let mut watch = watch();
    assert_eq!(watch.step(polled("0:1:1:"), 1_000), vec![]);
    assert_eq!(
        watch.step(polled("1:0:1:mux"), 1_250),
        vec![came_back("mux", 1_000)],
        "the client is back on the session, but not on the TUI's window"
    );
    assert_eq!(
        watch.step(polled("1:1:1:mux"), 1_500),
        vec![Signal::Regained],
        "the later window change reads the same stale session; nothing new"
    );
}

#[test]
fn a_return_from_no_session_or_without_a_socket_credits_nothing() {
    let mut watch = watch();
    watch.step(polled("0:1:1:"), 1_000);
    assert_eq!(
        watch.step(polled("1:1:1:"), 1_250),
        vec![Signal::Regained],
        "a client that never switched sessions has none to credit"
    );
    let mut unknown = Watch::new(None);
    unknown.step(polled("0:1:1:"), 1_000);
    assert_eq!(
        unknown.step(polled("1:1:1:mux"), 1_250),
        vec![Signal::Regained],
        "with no server to name, a session name means nothing"
    );
}

#[test]
fn the_tmux_variable_names_the_server_socket_of_the_environment_value() {
    assert_eq!(
        server_socket("/tmp/tmux-1000/default,4242,0"),
        Some("/tmp/tmux-1000/default".to_string())
    );
    assert_eq!(server_socket(",4242,0"), None);
    assert_eq!(server_socket(""), None);
}
