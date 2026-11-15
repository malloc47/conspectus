//! Which program an installed hook runs, and which one `hook init`
//! installs (ADR 0240).
//!
//! A hook command names the program by an absolute path, because the
//! harness runs it with whatever `PATH` it started with. The running
//! binary's own path is the obvious choice, but a Nix store path goes
//! away at the next upgrade or garbage collection, and a cargo build
//! directory at the next `cargo clean`; every hook then fails on every
//! event. So when the running binary sits in one of those, `hook init`
//! looks for a path that stays and runs the same binary: the
//! `conspectus` that `PATH` finds, then a Nix profile's link. Failing
//! both it installs the running binary's path and says why.
//!
//! `hook status` names an installed program that no longer exists, and
//! `hook init` moves the entries off one, or off a store or build path
//! once a path that stays runs this build.

use std::ffi::OsString;
use std::fs;
use std::io::{self, Read};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

/// The file name the stable candidates are looked up by.
const PROGRAM_NAME: &str = "conspectus";

/// Where a binary sits that an upgrade or a clean can remove.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Fragile {
    /// The Nix store (`/nix/store`, or `$NIX_STORE_DIR`).
    NixStore,
    /// A cargo build directory (a `target/debug` or `target/release`,
    /// or any directory under one holding a `CACHEDIR.TAG`).
    Build,
}

impl Fragile {
    pub(super) fn place(self) -> &'static str {
        match self {
            Fragile::NixStore => {
                "in the Nix store, which an upgrade or garbage collection can remove"
            }
            Fragile::Build => "in a cargo build directory, which `cargo clean` can empty",
        }
    }
}

/// Why `hook init` picked the program it installs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Why {
    /// The running binary, which sits where it stays.
    Running,
    /// The `conspectus` `PATH` finds, which is the running binary.
    OnPath,
    /// A Nix profile's `conspectus`, which is the running binary.
    Profile,
    /// The running binary, though it sits somewhere fragile, since no
    /// path that stays runs it.
    Fallback(Fragile),
}

/// The program `hook init` installs and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Choice {
    pub(super) program: PathBuf,
    pub(super) why: Why,
}

impl Choice {
    /// The hook command for `harness`.
    pub(super) fn command(&self, harness: &str) -> String {
        let program = self
            .program
            .to_str()
            .map_or_else(|| PROGRAM_NAME.to_string(), str::to_string);
        format!("{} hook write {harness}", shell_quote(&program))
    }

    /// What `hook init` prints about the choice.
    pub(super) fn describe(&self) -> String {
        let why = match &self.why {
            Why::Running => "this binary".to_string(),
            Why::OnPath => format!("the {PROGRAM_NAME} on PATH, which is this build"),
            Why::Profile => format!("the {PROGRAM_NAME} in your Nix profile, which is this build"),
            Why::Fallback(Fragile::NixStore) => "this binary, in the Nix store".to_string(),
            Why::Fallback(Fragile::Build) => "this binary, in a cargo build directory".to_string(),
        };
        format!("hooks run {} ({why})", self.program.display())
    }

    /// The warning for a fragile fallback, naming `init_command` as the
    /// fix after an upgrade.
    pub(super) fn warning(&self, init_command: &str) -> Option<String> {
        let Why::Fallback(fragile) = self.why else {
            return None;
        };
        Some(format!(
            "{} is {}, and no {PROGRAM_NAME} on PATH or in a Nix profile is this build; \
             once it is gone every hook fails until you run `{init_command}` again \
             (or pass --command with a path that stays)",
            self.program.display(),
            fragile.place()
        ))
    }
}

/// The program `hook init` installs when the running binary is `exe`
/// and the environment is `env`; see the module documentation.
pub(super) fn choose(exe: &Path, env: &impl Fn(&str) -> Option<OsString>) -> Choice {
    let Some(running) = fragile(exe, env) else {
        return Choice {
            program: exe.to_path_buf(),
            why: Why::Running,
        };
    };
    let runs_this = |candidate: &Path| {
        is_executable(candidate) && fragile(candidate, env).is_none() && same_binary(candidate, exe)
    };
    if let Some(found) = path_candidates(env).into_iter().find(|c| runs_this(c)) {
        return Choice {
            program: found,
            why: Why::OnPath,
        };
    }
    if let Some(found) = profile_candidates(env).into_iter().find(|c| runs_this(c)) {
        return Choice {
            program: found,
            why: Why::Profile,
        };
    }
    Choice {
        program: exe.to_path_buf(),
        why: Why::Fallback(running),
    }
}

/// Where `path`, an absolute path, sits that an upgrade or a clean can
/// remove, if anywhere. `None` for a relative path or a bare name.
pub(super) fn fragile(path: &Path, env: &impl Fn(&str) -> Option<OsString>) -> Option<Fragile> {
    if !path.is_absolute() {
        return None;
    }
    let store = env("NIX_STORE_DIR")
        .filter(|dir| !dir.is_empty())
        .map_or_else(|| PathBuf::from("/nix/store"), PathBuf::from);
    if path.starts_with(&store) {
        return Some(Fragile::NixStore);
    }
    let in_build = path.ancestors().skip(1).any(|dir| {
        let cargo_profile = dir
            .file_name()
            .is_some_and(|name| name == "debug" || name == "release")
            && dir
                .parent()
                .and_then(Path::file_name)
                .is_some_and(|name| name == "target");
        cargo_profile || dir.join("CACHEDIR.TAG").is_file()
    });
    in_build.then_some(Fragile::Build)
}

/// `conspectus` in each absolute `PATH` directory, in order.
fn path_candidates(env: &impl Fn(&str) -> Option<OsString>) -> Vec<PathBuf> {
    env("PATH")
        .map(|path| {
            std::env::split_paths(&path)
                .filter(|dir| dir.is_absolute())
                .map(|dir| dir.join(PROGRAM_NAME))
                .collect()
        })
        .unwrap_or_default()
}

/// `conspectus` in the Nix profiles a user's packages are linked from:
/// `~/.nix-profile`, its target under `~/.local/state/nix/profiles`, the
/// per-user profile NixOS and nix-darwin keep, and the NixOS system
/// profile.
fn profile_candidates(env: &impl Fn(&str) -> Option<OsString>) -> Vec<PathBuf> {
    let non_empty = |key: &str| env(key).filter(|value| !value.is_empty());
    let mut dirs = Vec::new();
    if let Some(home) = non_empty("HOME").map(PathBuf::from) {
        dirs.push(home.join(".nix-profile/bin"));
        dirs.push(home.join(".local/state/nix/profiles/profile/bin"));
    }
    if let Some(user) = non_empty("USER") {
        dirs.push(
            PathBuf::from("/etc/profiles/per-user")
                .join(user)
                .join("bin"),
        );
    }
    dirs.push(PathBuf::from("/run/current-system/sw/bin"));
    dirs.into_iter().map(|dir| dir.join(PROGRAM_NAME)).collect()
}

/// Whether `a` and `b` are the same binary: the same file once links are
/// resolved, or files of the same content.
fn same_binary(a: &Path, b: &Path) -> bool {
    match (fs::canonicalize(a), fs::canonicalize(b)) {
        (Ok(a), Ok(b)) if a == b => true,
        (Ok(a), Ok(b)) => same_content(&a, &b).unwrap_or(false),
        _ => false,
    }
}

fn same_content(a: &Path, b: &Path) -> io::Result<bool> {
    if fs::metadata(a)?.len() != fs::metadata(b)?.len() {
        return Ok(false);
    }
    let (mut a, mut b) = (fs::File::open(a)?, fs::File::open(b)?);
    let (mut left, mut right) = (vec![0u8; 64 * 1024], vec![0u8; 64 * 1024]);
    loop {
        let read = a.read(&mut left)?;
        if read == 0 {
            return Ok(true);
        }
        b.read_exact(&mut right[..read])?;
        if left[..read] != right[..read] {
            return Ok(false);
        }
    }
}

fn is_executable(path: &Path) -> bool {
    fs::metadata(path).is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
}

/// How an installed hook's program stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Program {
    /// A path to a file that exists, or a name `PATH` finds.
    Found,
    /// A path to nothing: every hook fails.
    Gone,
    /// A bare name this `PATH` does not find. The harness looks it up on
    /// the `PATH` it started with, which may differ.
    NotOnPath,
}

/// How `program`, as a hook command names it, stands in `env`.
pub(super) fn standing(program: &str, env: &impl Fn(&str) -> Option<OsString>) -> Program {
    if program.contains('/') {
        return if Path::new(program).is_file() {
            Program::Found
        } else {
            Program::Gone
        };
    }
    let found = env("PATH").is_some_and(|path| {
        std::env::split_paths(&path).any(|dir| is_executable(&dir.join(program)))
    });
    if found {
        Program::Found
    } else {
        Program::NotOnPath
    }
}

/// The program a shell command line runs: its first word, unquoted,
/// after any leading `NAME=value` assignments and an `env` that sets
/// them. `None` for a line with no word.
pub(super) fn program_of(line: &str) -> Option<String> {
    let mut rest = line;
    let mut after_env = false;
    loop {
        let (word, tail) = first_word(rest)?;
        rest = tail;
        if is_assignment(&word) {
            continue;
        }
        if word == "env" && !after_env {
            after_env = true;
            continue;
        }
        return Some(word);
    }
}

fn is_assignment(word: &str) -> bool {
    word.split_once('=').is_some_and(|(name, _)| {
        !name.is_empty()
            && !name.starts_with(|c: char| c.is_ascii_digit())
            && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    })
}

/// The first shell word of `line`, with quotes and escapes removed, and
/// the text after it.
fn first_word(line: &str) -> Option<(String, &str)> {
    let line = line.trim_start();
    let mut word = String::new();
    let mut chars = line.char_indices().peekable();
    let mut started = false;
    while let Some((index, c)) = chars.next() {
        match c {
            '\'' => {
                started = true;
                for (_, inner) in chars.by_ref() {
                    if inner == '\'' {
                        break;
                    }
                    word.push(inner);
                }
            }
            '"' => {
                started = true;
                while let Some((_, inner)) = chars.next() {
                    match inner {
                        '"' => break,
                        '\\' => {
                            if let Some(&(_, next)) = chars.peek()
                                && matches!(next, '"' | '\\' | '$' | '`')
                            {
                                word.push(next);
                                chars.next();
                            } else {
                                word.push('\\');
                            }
                        }
                        other => word.push(other),
                    }
                }
            }
            '\\' => {
                started = true;
                if let Some((_, next)) = chars.next() {
                    word.push(next);
                }
            }
            c if c.is_whitespace() || matches!(c, ';' | '|' | '&' | '<' | '>' | '(' | ')') => {
                return started.then(|| (word, &line[index..]));
            }
            other => {
                started = true;
                word.push(other);
            }
        }
    }
    started.then_some((word, ""))
}

/// `value` as one shell word: as is when it holds only path-safe
/// characters, else single-quoted.
pub(super) fn shell_quote(value: &str) -> String {
    if !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'.' | b'_' | b'-'))
    {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

#[cfg(test)]
#[path = "hook_program_tests.rs"]
mod tests;
