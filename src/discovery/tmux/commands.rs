//! The tmux commands Conspectus starts, as argument lists.
//!
//! [`SystemTmux`](super::SystemTmux) runs exactly these arguments, and
//! the rebuild plan prints exactly these arguments, so what a plan shows
//! is what a rebuild runs and someone can type it by hand.

use std::ffi::OsString;
use std::path::Path;

use super::effective_socket;

/// The program name a plan shows. The binary a run uses can be
/// overridden, but the command someone types is `tmux`.
pub const TMUX_PROGRAM: &str = "tmux";

fn with_socket(socket: Option<&str>, rest: &[&str]) -> Vec<OsString> {
    let mut args = Vec::new();
    if let Some(socket) = effective_socket(socket) {
        args.push(OsString::from("-L"));
        args.push(OsString::from(socket));
    }
    args.extend(rest.iter().map(OsString::from));
    args
}

/// `new-session -d -s <name> -c <cwd> <argv…>`, chained with the option
/// that keeps a pane whose process exits non-zero, so a harness that
/// fails at startup leaves its error readable.
pub fn new_session_args(
    socket: Option<&str>,
    name: &str,
    cwd: &Path,
    argv: &[OsString],
) -> Vec<OsString> {
    let mut args = with_socket(socket, &["new-session", "-d", "-s", name]);
    args.push(OsString::from("-c"));
    args.push(cwd.as_os_str().to_os_string());
    args.extend(argv.iter().cloned());
    args.extend(
        [";", "set-option", "-t", name, "remain-on-exit", "failed"]
            .iter()
            .map(OsString::from),
    );
    args
}

/// `attach-session -t <name>`, or `switch-client` from inside tmux.
pub fn attach_args(socket: Option<&str>, name: &str, nested: bool) -> Vec<OsString> {
    let subcommand = if nested {
        "switch-client"
    } else {
        "attach-session"
    };
    with_socket(socket, &[subcommand, "-t", name])
}

/// `send-keys -t <target> <literal> [Enter]`.
pub fn send_keys_args(
    socket: Option<&str>,
    target: &str,
    literal: &str,
    press_enter: bool,
) -> Vec<OsString> {
    let mut args = with_socket(socket, &["send-keys", "-t", target, literal]);
    if press_enter {
        args.push(OsString::from("Enter"));
    }
    args
}

/// `kill-session -t <target>`.
pub fn kill_session_args(socket: Option<&str>, target: &str) -> Vec<OsString> {
    with_socket(socket, &["kill-session", "-t", target])
}
