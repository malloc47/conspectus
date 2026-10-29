//! The argv field the launch and pin edit forms share: the command an
//! empty field means, a harness's launch options in it, and how a typed
//! argv follows a change of harness.
//!
//! Both forms read the field with the one argv grammar (`crate::argv`).
//! While the operator edits, they read it as far as it goes
//! (`split_command_line`), so an option or a harness change works on
//! what is typed so far and writes it back finished; submitting parses it
//! strictly and refuses an unclosed quote. An argv is written back with
//! `argv::shell_quote`, which splits back to the same arguments.

use std::path::Path;

use crate::discovery::harness::{
    HarnessLaunchOption, argv_contains_fragment, argv_with_fragment, argv_without_fragment,
    launch_argv_for, strip_known_launch_option_fragments,
};
use crate::launch_default::{DefaultSource, LaunchDefaults};

/// The argv an empty argv field means for one harness started in one
/// directory, and where it comes from, kept until either changes.
///
/// Deciding it can read the disk (whether the directory is inside an
/// Atelier workspace), so a form keeps the answer, asks again only when
/// the harness or the directory changes ([`refresh`](Self::refresh)), and
/// draws from what it kept.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DefaultArgv {
    harness: String,
    cwd: String,
    argv: Vec<String>,
    source: Option<DefaultSource>,
}

impl DefaultArgv {
    /// Decides the default for `harness` started in `cwd`: the launch
    /// profile or `atelier exec` that `defaults` gives, else the harness's
    /// own default (empty for a harness Conspectus does not know).
    pub fn resolve(defaults: Option<&LaunchDefaults>, harness: &str, cwd: &str) -> Self {
        let (harness, cwd) = (harness.trim(), cwd.trim());
        let resolved = defaults.and_then(|defaults| {
            defaults.default_argv(harness, (!cwd.is_empty()).then(|| Path::new(cwd)))
        });
        let (argv, source) = match resolved {
            Some((argv, source)) => (argv, Some(source)),
            None => (launch_argv_for(harness), None),
        };
        Self {
            harness: harness.to_string(),
            cwd: cwd.to_string(),
            argv: argv
                .iter()
                .map(|arg| arg.to_string_lossy().into_owned())
                .collect(),
            source,
        }
    }

    /// Decides again when `harness` or `cwd` is not what the kept answer
    /// was decided for; whether it did.
    pub fn refresh(&mut self, defaults: Option<&LaunchDefaults>, harness: &str, cwd: &str) -> bool {
        if self.harness == harness.trim() && self.cwd == cwd.trim() {
            return false;
        }
        *self = Self::resolve(defaults, harness, cwd);
        true
    }

    /// The default argv; empty when the harness has none.
    pub fn argv(&self) -> &[String] {
        &self.argv
    }

    /// Where the default comes from, when launch defaults decided it.
    pub fn source(&self) -> Option<DefaultSource> {
        self.source
    }

    /// The default and where it comes from, as the launch form shows it:
    /// `claude --x — from the project launch profile`. `None` when launch
    /// defaults did not decide it.
    pub fn hint(&self) -> Option<String> {
        let source = self.source?;
        Some(format!("{} — from {}", self.argv.join(" "), source.label()))
    }
}

/// The argv a field holding `typed` launches: `typed`, or `default` when
/// nothing is typed.
pub fn effective(typed: Vec<String>, default: &[String]) -> Vec<String> {
    if typed.is_empty() {
        default.to_vec()
    } else {
        typed
    }
}

/// Whether `argv` carries each of `options`' flags, in order.
pub fn options_on(options: &[HarnessLaunchOption], argv: &[String]) -> Vec<bool> {
    options
        .iter()
        .map(|option| argv_contains_fragment(argv, option.argv))
        .collect()
}

/// `argv` with `option`'s flags added when `on`, taken out when not.
pub fn with_option(argv: Vec<String>, option: HarnessLaunchOption, on: bool) -> Vec<String> {
    if on {
        argv_with_fragment(argv, option.argv)
    } else {
        argv_without_fragment(argv, option.argv)
    }
}

/// A typed argv carried to a new harness: every harness's launch-option
/// flags come off it, and one that was then only the old harness's
/// default becomes the new harness's default.
pub fn carried(typed: Vec<String>, old_default: &[String], new_default: &[String]) -> Vec<String> {
    let stripped = strip_known_launch_option_fragments(typed);
    if stripped == old_default {
        new_default.to_vec()
    } else {
        stripped
    }
}

/// `argv` written the way a shell would take it back.
pub fn display(argv: &[String]) -> String {
    argv.iter()
        .map(|arg| crate::argv::shell_quote(std::ffi::OsStr::new(arg)))
        .collect::<Vec<_>>()
        .join(" ")
}

/// The text an argv field holds for `argv`: empty when `argv` is just
/// `default` (or nothing), so the field keeps following the default.
pub fn field_text(argv: &[String], default: &[String]) -> String {
    if argv.is_empty() || argv == default {
        String::new()
    } else {
        display(argv)
    }
}

#[cfg(test)]
#[path = "launch_argv_tests.rs"]
mod tests;
