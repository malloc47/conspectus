//! Runs the built binary under `strace` and checks everything it does
//! against `docs/capabilities.toml` (ADR 0117): the programs it starts
//! and their arguments, the sockets it opens, the signals it sends, and
//! the files it writes.
//!
//! Each scenario runs one or more commands against a materialized
//! showcase world, with a temporary home and XDG directories and
//! stand-in `gh`, `tmux`, `zellij`, and `wt` scripts on `PATH`, so the
//! run is deterministic and never touches the network or a real tmux
//! server. Only what the Conspectus process itself does is checked;
//! what the programs it starts do is theirs.
//!
//! Without `strace` (or when ptrace isn't permitted) the test skips,
//! unless `CONSPECTUS_REQUIRE_STRACE` is set, as CI sets it. Set
//! `CONSPECTUS_CAPABILITY_REPORT` to print every event each scenario
//! recorded.

#![cfg(target_os = "linux")]

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::ffi::OsString;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const BINARY: &str = env!("CARGO_BIN_EXE_conspectus");

/// The system calls the checks read.
const TRACED: &str = "trace=execve,execveat,clone,clone3,fork,vfork,socket,connect,bind,kill,\
                      openat,open,creat,rename,renameat,renameat2,unlink,unlinkat,mkdir,mkdirat,\
                      rmdir";

// ---------------------------------------------------------------------
// The manifest
// ---------------------------------------------------------------------

struct ProgramRule {
    key: String,
    names: Vec<String>,
    env: Option<String>,
    is_self: bool,
    globals: BTreeMap<String, usize>,
    chain: Option<String>,
    /// Argument patterns by context.
    patterns: BTreeMap<String, Vec<Vec<String>>>,
}

struct Manifest {
    programs: Vec<ProgramRule>,
    families: Vec<String>,
    binds: Vec<String>,
    connects: Vec<String>,
    /// Signals by context.
    signals: BTreeMap<String, Vec<String>>,
    /// Write patterns by context, `always` included.
    writes: BTreeMap<String, Vec<String>>,
}

const CONTEXTS: [&str; 4] = ["read", "act", "hook", "dev"];

fn strings(value: Option<&toml::Value>) -> Vec<String> {
    value
        .and_then(toml::Value::as_array)
        .map(|values| {
            values
                .iter()
                .map(|value| value.as_str().expect("a string").to_string())
                .collect()
        })
        .unwrap_or_default()
}

fn load_manifest() -> Manifest {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/capabilities.toml");
    let table: toml::Table = fs::read_to_string(path)
        .expect("read manifest")
        .parse()
        .expect("parse manifest");
    let programs = table["programs"]
        .as_table()
        .expect("[programs]")
        .iter()
        .map(|(key, rule)| {
            let globals = rule
                .get("globals")
                .and_then(toml::Value::as_table)
                .map(|globals| {
                    globals
                        .iter()
                        .map(|(flag, arity)| {
                            let arity = arity.as_integer().expect("arity");
                            (flag.clone(), usize::try_from(arity).expect("arity"))
                        })
                        .collect()
                })
                .unwrap_or_default();
            let patterns = CONTEXTS
                .iter()
                .map(|context| {
                    let patterns = strings(rule.get(*context))
                        .iter()
                        .map(|pattern| pattern.split_whitespace().map(str::to_string).collect())
                        .collect();
                    ((*context).to_string(), patterns)
                })
                .collect();
            ProgramRule {
                key: key.clone(),
                names: strings(rule.get("names")),
                env: rule.get("env").and_then(|v| v.as_str()).map(str::to_string),
                is_self: rule.get("self").and_then(toml::Value::as_bool) == Some(true),
                globals,
                chain: rule
                    .get("chain")
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
                patterns,
            }
        })
        .collect();
    let section = |name: &str, key: &str| strings(table[name].get(key));
    let by_context = |name: &str, extra: &[&str]| {
        CONTEXTS
            .iter()
            .chain(extra)
            .map(|context| ((*context).to_string(), section(name, context)))
            .collect()
    };
    Manifest {
        programs,
        families: section("sockets", "families"),
        binds: section("sockets", "bind"),
        connects: section("sockets", "connect"),
        signals: by_context("signals", &[]),
        writes: by_context("writes", &["always"]),
    }
}

/// Whether `args` match a pattern: literal words, `*` for any one word,
/// and a trailing `...` for any remaining arguments.
fn pattern_matches(pattern: &[String], args: &[String]) -> bool {
    let (words, open) = match pattern.split_last() {
        Some((last, rest)) if last == "..." => (rest, true),
        _ => (pattern, false),
    };
    if args.len() < words.len() || (!open && args.len() != words.len()) {
        return false;
    }
    words
        .iter()
        .zip(args)
        .all(|(word, arg)| word == "*" || word == arg)
}

/// Whether `text` matches `pattern`, where `*` matches any run of
/// characters.
fn wildcard(pattern: &str, text: &str) -> bool {
    match pattern.split_once('*') {
        None => pattern == text,
        Some((head, tail)) => {
            let Some(rest) = text.strip_prefix(head) else {
                return false;
            };
            (0..=rest.len())
                .filter(|at| rest.is_char_boundary(*at))
                .any(|at| wildcard(tail, &rest[at..]))
        }
    }
}

/// Whether a path matches a glob of components, where a `**` component
/// matches any number of components.
fn glob(pattern: &[&str], path: &[&str]) -> bool {
    match pattern.split_first() {
        None => path.is_empty(),
        Some((&"**", rest)) => (0..=path.len()).any(|skip| glob(rest, &path[skip..])),
        Some((first, rest)) => path
            .split_first()
            .is_some_and(|(component, tail)| wildcard(first, component) && glob(rest, tail)),
    }
}

fn components(path: &str) -> Vec<&str> {
    path.split('/').filter(|part| !part.is_empty()).collect()
}

// ---------------------------------------------------------------------
// The trace
// ---------------------------------------------------------------------

/// Something the Conspectus process did.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Event {
    Exec { path: String, argv: Vec<String> },
    Socket { family: String },
    Connect { family: String, path: String },
    Bind { family: String, path: String },
    Signal { signal: String },
    Write { syscall: String, path: String },
}

/// Unescape a C string literal starting at `text[0] == '"'`, returning
/// the string and the number of bytes consumed.
fn c_string(text: &str) -> (String, usize) {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut at = 1;
    while at < bytes.len() {
        match bytes[at] {
            b'"' => {
                at += 1;
                // A string cut at the length limit ends in `...`.
                if text[at..].starts_with("...") {
                    at += 3;
                }
                break;
            }
            b'\\' if at + 1 < bytes.len() => {
                at += 1;
                match bytes[at] {
                    b'n' => out.push(b'\n'),
                    b't' => out.push(b'\t'),
                    b'r' => out.push(b'\r'),
                    b'v' => out.push(0x0b),
                    b'f' => out.push(0x0c),
                    b'x' => {
                        let hex = &text[at + 1..(at + 3).min(text.len())];
                        out.push(u8::from_str_radix(hex, 16).unwrap_or(b'?'));
                        at += 2;
                    }
                    b'0'..=b'7' => {
                        let end = (at..(at + 3).min(bytes.len()))
                            .take_while(|i| (b'0'..=b'7').contains(&bytes[*i]))
                            .last()
                            .map_or(at, |i| i + 1);
                        out.push(u8::from_str_radix(&text[at..end], 8).unwrap_or(b'?'));
                        at = end - 1;
                    }
                    other => out.push(other),
                }
                at += 1;
            }
            other => {
                out.push(other);
                at += 1;
            }
        }
    }
    (String::from_utf8_lossy(&out).into_owned(), at)
}

/// Split a syscall's argument list at top-level commas.
fn split_args(args: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut depth = 0i32;
    let mut chars = args.char_indices().peekable();
    while let Some((at, ch)) = chars.next() {
        match ch {
            '"' => {
                let (_, used) = c_string(&args[at..]);
                current.push_str(&args[at..at + used]);
                while chars.peek().is_some_and(|(next, _)| *next < at + used) {
                    chars.next();
                }
            }
            '[' | '{' | '(' | '<' => {
                depth += 1;
                current.push(ch);
            }
            ']' | '}' | ')' | '>' => {
                depth -= 1;
                current.push(ch);
            }
            ',' if depth == 0 => {
                out.push(current.trim().to_string());
                current.clear();
            }
            _ => current.push(ch),
        }
    }
    if !current.trim().is_empty() {
        out.push(current.trim().to_string());
    }
    out
}

/// A traced call: its name, arguments, and return text.
struct Call {
    name: String,
    args: Vec<String>,
    result: String,
}

fn parse_call(line: &str) -> Option<Call> {
    let open = line.find('(')?;
    let name = &line[..open];
    if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return None;
    }
    // Find the closing parenthesis at depth zero, skipping strings.
    let body = &line[open + 1..];
    let mut depth = 0i32;
    let mut close = None;
    let mut at = 0;
    while at < body.len() {
        match body.as_bytes()[at] {
            b'"' => {
                at += c_string(&body[at..]).1;
                continue;
            }
            b'(' | b'[' | b'{' => depth += 1,
            b']' | b'}' => depth -= 1,
            b')' if depth == 0 => {
                close = Some(at);
                break;
            }
            b')' => depth -= 1,
            _ => {}
        }
        at += 1;
    }
    let close = close?;
    let result = body[close + 1..]
        .trim_start()
        .strip_prefix('=')?
        .trim()
        .to_string();
    Some(Call {
        name: name.to_string(),
        args: split_args(&body[..close]),
        result,
    })
}

fn quoted(arg: &str) -> Option<String> {
    arg.starts_with('"').then(|| c_string(arg).0)
}

/// The path a decorated descriptor names: `3</a/b>` or `AT_FDCWD</a>`.
fn decoration(arg: &str) -> Option<String> {
    let start = arg.find('<')?;
    let end = arg.rfind('>')?;
    (end > start).then(|| arg[start + 1..end].to_string())
}

/// `path` resolved against the directory descriptor `dirfd`.
fn resolve(dirfd: Option<&str>, path: &str, cwd: &Path) -> String {
    if path.starts_with('/') {
        return path.to_string();
    }
    let base = dirfd
        .and_then(decoration)
        .map_or_else(|| cwd.to_path_buf(), PathBuf::from);
    base.join(path).to_string_lossy().into_owned()
}

fn string_array(arg: &str) -> Vec<String> {
    let inner = arg
        .trim()
        .strip_prefix('[')
        .and_then(|rest| rest.strip_suffix(']'))
        .unwrap_or("");
    split_args(inner)
        .iter()
        .filter_map(|item| quoted(item))
        .collect()
}

const WRITE_FLAGS: [&str; 5] = ["O_WRONLY", "O_RDWR", "O_CREAT", "O_TRUNC", "O_APPEND"];

fn writes_with(flags: &str) -> bool {
    flags
        .split('|')
        .any(|flag| WRITE_FLAGS.contains(&flag.trim()))
}

fn sockaddr(arg: &str) -> (String, String) {
    let field = |name: &str| {
        arg.find(&format!("{name}="))
            .map(|at| &arg[at + name.len() + 1..])
            .map(|rest| match quoted(rest) {
                Some(value) => value,
                None => rest
                    .split([',', '}'])
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string(),
            })
            .unwrap_or_default()
    };
    (field("sa_family"), field("sun_path"))
}

/// The events a call records when the Conspectus process makes it.
fn events(call: &Call, cwd: &Path) -> Vec<Event> {
    let arg = |index: usize| call.args.get(index).map_or("", String::as_str);
    let path_at = |dirfd: Option<usize>, index: usize| {
        quoted(arg(index)).map(|path| resolve(dirfd.map(arg), &path, cwd))
    };
    let write = |path: Option<String>| {
        path.map(|path| Event::Write {
            syscall: call.name.clone(),
            path,
        })
    };
    match call.name.as_str() {
        "execve" => vec![Event::Exec {
            path: quoted(arg(0)).unwrap_or_default(),
            argv: string_array(arg(1)),
        }],
        "execveat" => vec![Event::Exec {
            path: path_at(Some(0), 1).unwrap_or_default(),
            argv: string_array(arg(2)),
        }],
        "socket" => vec![Event::Socket {
            family: arg(0).to_string(),
        }],
        "connect" | "bind" => {
            let (family, path) = sockaddr(arg(1));
            if call.name == "connect" {
                vec![Event::Connect { family, path }]
            } else {
                vec![Event::Bind { family, path }]
            }
        }
        "kill" => vec![Event::Signal {
            signal: arg(1).to_string(),
        }],
        "openat" if writes_with(arg(2)) => {
            // A successful open reports the file it opened.
            let opened = call
                .result
                .split_whitespace()
                .next()
                .and_then(decoration)
                .filter(|path| path.starts_with('/'));
            write(opened.or_else(|| path_at(Some(0), 1)))
                .into_iter()
                .collect()
        }
        "open" if writes_with(arg(1)) => write(path_at(None, 0)).into_iter().collect(),
        "creat" | "mkdir" | "rmdir" | "unlink" => write(path_at(None, 0)).into_iter().collect(),
        "mkdirat" | "unlinkat" => write(path_at(Some(0), 1)).into_iter().collect(),
        "rename" => [path_at(None, 0), path_at(None, 1)]
            .into_iter()
            .filter_map(write)
            .collect(),
        "renameat" | "renameat2" => [path_at(Some(0), 1), path_at(Some(2), 3)]
            .into_iter()
            .filter_map(write)
            .collect(),
        _ => Vec::new(),
    }
}

/// Every event the Conspectus processes recorded in a `strace -ff`
/// output directory. Each thread has its own file; a thread is
/// Conspectus while its process runs the Conspectus binary, which it
/// inherits from the thread that cloned it until it `execve`s.
fn conspectus_events(dir: &Path, cwd: &Path) -> BTreeSet<Event> {
    let binary = fs::canonicalize(BINARY).expect("canonical binary path");
    let is_conspectus = |path: &str| fs::canonicalize(path).is_ok_and(|path| path == binary);

    let mut files: BTreeMap<u64, Vec<String>> = BTreeMap::new();
    for entry in fs::read_dir(dir).expect("read trace dir").flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(tid) = name.rsplit('.').next().and_then(|tid| tid.parse().ok()) else {
            continue;
        };
        let text = fs::read_to_string(entry.path()).expect("read trace");
        files.insert(tid, text.lines().map(str::to_string).collect());
    }
    let mut children: BTreeSet<u64> = BTreeSet::new();
    for lines in files.values() {
        for call in lines.iter().filter_map(|line| parse_call(line)) {
            if matches!(call.name.as_str(), "clone" | "clone3" | "fork" | "vfork")
                && let Ok(child) = call.result.parse::<u64>()
            {
                children.insert(child);
            }
        }
    }
    let roots: Vec<u64> = files
        .keys()
        .filter(|tid| !children.contains(tid))
        .copied()
        .collect();
    assert_eq!(roots.len(), 1, "one traced root in {}", dir.display());

    let mut out = BTreeSet::new();
    let mut queue: VecDeque<(u64, bool)> = VecDeque::from([(roots[0], false)]);
    while let Some((tid, mut conspectus)) = queue.pop_front() {
        for call in files
            .get(&tid)
            .into_iter()
            .flatten()
            .filter_map(|line| parse_call(line))
        {
            if conspectus {
                out.extend(events(&call, cwd));
            }
            match call.name.as_str() {
                "execve" | "execveat" if call.result == "0" => {
                    let path = match events(&call, cwd).pop() {
                        Some(Event::Exec { path, .. }) => path,
                        _ => String::new(),
                    };
                    conspectus = is_conspectus(&path);
                }
                "clone" | "clone3" | "fork" | "vfork" => {
                    if let Ok(child) = call.result.parse::<u64>() {
                        queue.push_back((child, conspectus));
                    }
                }
                _ => {}
            }
        }
    }
    out
}

// ---------------------------------------------------------------------
// Checking events against the manifest
// ---------------------------------------------------------------------

/// Path variables for one run.
struct Vars(BTreeMap<&'static str, Vec<String>>);

impl Vars {
    fn expand(&self, pattern: &str) -> Vec<String> {
        let Some(rest) = pattern.strip_prefix('$') else {
            return vec![pattern.to_string()];
        };
        let (name, tail) = rest.split_once('/').unwrap_or((rest, ""));
        self.0
            .get(name)
            .into_iter()
            .flatten()
            .map(|value| {
                if tail.is_empty() {
                    value.clone()
                } else {
                    format!("{value}/{tail}")
                }
            })
            .collect()
    }

    fn matches(&self, patterns: &[String], path: &str) -> bool {
        patterns
            .iter()
            .flat_map(|p| self.expand(p))
            .any(|pattern| glob(&components(&pattern), &components(path)))
    }

    /// Whether `path` is a directory above an allowed path, which
    /// creating that path's missing parents makes.
    fn is_parent_of(&self, patterns: &[String], path: &str) -> bool {
        let path = components(path);
        patterns.iter().flat_map(|p| self.expand(p)).any(|pattern| {
            let literal: Vec<&str> = components(&pattern)
                .into_iter()
                .take_while(|part| !part.contains('*'))
                .collect();
            literal.len() > path.len() && literal.starts_with(&path)
        })
    }
}

struct Scenario<'a> {
    name: &'a str,
    contexts: &'a [&'a str],
    vars: &'a Vars,
    pager: Option<&'a str>,
}

fn allowed(by_context: &BTreeMap<String, Vec<String>>, contexts: &[&str]) -> Vec<String> {
    contexts
        .iter()
        .chain(&["always"])
        .flat_map(|context| by_context.get(*context).cloned().unwrap_or_default())
        .collect()
}

fn check_exec(
    manifest: &Manifest,
    scenario: &Scenario,
    path: &str,
    argv: &[String],
) -> Option<String> {
    let binary = fs::canonicalize(BINARY).expect("canonical binary path");
    let base = Path::new(path)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let is_self = fs::canonicalize(path).is_ok_and(|path| path == binary);
    let rule = manifest.programs.iter().find(|rule| {
        (rule.is_self && is_self)
            || rule.names.contains(&base)
            || (rule.env.is_some() && scenario.pager == Some(base.as_str()))
    });
    let Some(rule) = rule else {
        return Some(format!("runs `{path}`, which no [programs] entry names"));
    };
    // Skip the program name and its leading global options.
    let mut args = argv.get(1..).unwrap_or_default();
    while let Some(arity) = args.first().and_then(|flag| rule.globals.get(flag)) {
        args = args.get(1 + arity..).unwrap_or_default();
    }
    let segments: Vec<&[String]> = match &rule.chain {
        Some(chain) => args.split(|arg| arg == chain).collect(),
        None => vec![args],
    };
    for segment in segments {
        let fits = |contexts: &[&str]| {
            contexts.iter().any(|context| {
                rule.patterns
                    .get(*context)
                    .is_some_and(|patterns| patterns.iter().any(|p| pattern_matches(p, segment)))
            })
        };
        if fits(scenario.contexts) {
            continue;
        }
        let elsewhere: Vec<&str> = CONTEXTS
            .iter()
            .copied()
            .filter(|context| fits(&[context]))
            .collect();
        let shown = segment.join(" ");
        return Some(if elsewhere.is_empty() {
            format!(
                "runs `{} {shown}`, which [programs.{}] doesn't allow",
                rule.key, rule.key
            )
        } else {
            format!(
                "runs `{} {shown}`, which [programs.{}] allows only in {}",
                rule.key,
                rule.key,
                elsewhere.join(", ")
            )
        });
    }
    None
}

fn check(manifest: &Manifest, scenario: &Scenario, event: &Event) -> Option<String> {
    let vars = scenario.vars;
    match event {
        Event::Exec { path, argv } => check_exec(manifest, scenario, path, argv),
        Event::Socket { family } => (!manifest.families.contains(family)).then(|| {
            format!(
                "opens an {family} socket; [sockets] allows only {:?}",
                manifest.families
            )
        }),
        Event::Connect { family, path } => (family == "AF_UNIX"
            && !vars.matches(&manifest.connects, path))
        .then(|| format!("connects to the Unix socket `{path}`, which [sockets] doesn't list")),
        Event::Bind { family, path } => (family == "AF_UNIX"
            && !vars.matches(&manifest.binds, path))
        .then(|| format!("binds the Unix socket `{path}`, which [sockets] doesn't list")),
        Event::Signal { signal } => (!allowed(&manifest.signals, scenario.contexts)
            .contains(signal))
        .then(|| format!("sends {signal}, which [signals] doesn't allow here")),
        Event::Write { syscall, path } => {
            let patterns = allowed(&manifest.writes, scenario.contexts);
            let ok = vars.matches(&patterns, path)
                || (matches!(syscall.as_str(), "mkdir" | "mkdirat")
                    && vars.is_parent_of(&patterns, path));
            (!ok).then(|| format!("{syscall} `{path}`, which [writes] doesn't allow here"))
        }
    }
}

// ---------------------------------------------------------------------
// The test world
// ---------------------------------------------------------------------

/// `strace`, when it's installed and allowed to trace.
fn strace() -> Option<PathBuf> {
    let found = std::env::var_os("PATH").and_then(|path| {
        std::env::split_paths(&path)
            .map(|dir| dir.join("strace"))
            .find(|candidate| candidate.is_file())
    });
    let usable = found.filter(|strace| {
        Command::new(strace)
            .args(["-o", "/dev/null", "true"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    });
    if usable.is_none() {
        assert!(
            std::env::var_os("CONSPECTUS_REQUIRE_STRACE").is_none(),
            "CONSPECTUS_REQUIRE_STRACE is set, but strace is missing or can't trace"
        );
        eprintln!("skipping the capability trace: strace is missing or can't trace");
    }
    usable
}

fn which(program: &str) -> PathBuf {
    let path = std::env::var_os("PATH").expect("PATH");
    std::env::split_paths(&path)
        .map(|dir| dir.join(program))
        .find(|candidate| candidate.is_file())
        .unwrap_or_else(|| panic!("{program} on PATH"))
}

/// Stand-in programs: none of them touch the network or a real server.
const SHIMS: &[(&str, &str)] = &[
    (
        "gh",
        "case \"$1 $2\" in\n  \"pr list\") echo '[]' ;;\n  \"pr view\") echo '{}' ;;\n  \"auth status\") echo '{\"hosts\":{}}' ;;\nesac\n",
    ),
    (
        "tmux",
        "# Answers like a tmux with no server, except for the session\n\
         # lifecycle commands, which succeed.\n\
         for arg in \"$@\"; do\n  case \"$arg\" in\n    \
         new-session|set-option|rename-session|kill-session) exit 0 ;;\n  esac\n\
         done\necho \"no server running on /tmp/tmux-test/default\" >&2\nexit 1\n",
    ),
    (
        "zellij",
        "echo 'No active zellij sessions found.' >&2\nexit 1\n",
    ),
    ("wt", "exit 0\n"),
];

struct World {
    _root: tempfile::TempDir,
    root: PathBuf,
    world: PathBuf,
    project: PathBuf,
    env: Vec<(&'static str, OsString)>,
    vars_base: BTreeMap<&'static str, Vec<String>>,
}

impl World {
    fn new() -> Self {
        let root_dir = tempfile::tempdir().expect("temp dir");
        let root = fs::canonicalize(root_dir.path()).expect("canonical root");
        let bin = root.join("bin");
        fs::create_dir_all(&bin).expect("bin dir");
        for (name, body) in SHIMS {
            let path = bin.join(name);
            fs::write(&path, format!("#!/bin/sh\n{body}")).expect("write shim");
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("chmod shim");
        }
        let git_dir = which("git").parent().expect("git dir").to_path_buf();
        let path = std::env::join_paths([bin, git_dir]).expect("PATH");

        let dir = |name: &str| root.join(name);
        let home = dir("home");
        fs::create_dir_all(&home).expect("home");
        let world = dir("world");
        let mut env: Vec<(&'static str, OsString)> = vec![
            ("PATH", path),
            ("HOME", home.clone().into()),
            ("XDG_CONFIG_HOME", dir("xdg/config").into()),
            ("XDG_STATE_HOME", dir("xdg/state").into()),
            ("XDG_CACHE_HOME", dir("xdg/cache").into()),
            ("XDG_DATA_HOME", dir("xdg/data").into()),
            ("XDG_RUNTIME_DIR", dir("xdg/run").into()),
            ("NO_COLOR", "1".into()),
            (
                "CONSPECTUS_CLAUDE_CODE_STATE",
                world.join("harness/claude").into(),
            ),
            ("CONSPECTUS_CODEX_STATE", world.join("harness/codex").into()),
            (
                "CONSPECTUS_OPENCODE_STATE",
                world.join("harness/opencode").into(),
            ),
            ("CONSPECTUS_PI_STATE", world.join("harness/pi").into()),
            (
                "CONSPECTUS_AGENT_DECK_ROOT",
                world.join(".agent-deck").into(),
            ),
        ];
        env.sort();
        let text = |path: &Path| path.to_string_lossy().into_owned();
        let project = world.join("repos/project");
        let vars_base = BTreeMap::from([
            ("HOME", vec![text(&home)]),
            ("CONFIG", vec![text(&dir("xdg/config"))]),
            ("STATE", vec![text(&dir("xdg/state"))]),
            ("CACHE", vec![text(&dir("xdg/cache"))]),
            ("DATA", vec![text(&dir("xdg/data"))]),
            ("RUNTIME", vec![text(&dir("xdg/run"))]),
            ("PROJECT", vec![text(&project)]),
        ]);
        Self {
            _root: root_dir,
            root,
            world,
            project,
            env,
            vars_base,
        }
    }

    fn vars(&self, scratch: Option<&Path>) -> Vars {
        let mut vars = self.vars_base.clone();
        if let Some(scratch) = scratch {
            vars.insert("SCRATCH", vec![scratch.to_string_lossy().into_owned()]);
        }
        Vars(vars)
    }

    /// Start `conspectus <args>` under strace, tracing into a new
    /// directory.
    fn spawn(&self, strace: &Path, label: &str, args: &[&str], cwd: &Path) -> (Child, PathBuf) {
        let dir = self.root.join("traces").join(label);
        fs::create_dir_all(&dir).expect("trace dir");
        let child = Command::new(strace)
            .args([
                "-ff",
                "-qq",
                "-y",
                "-s",
                "4096",
                "-e",
                "signal=none",
                "-e",
                TRACED,
                "-o",
            ])
            .arg(dir.join("t"))
            .arg(BINARY)
            .args(args)
            .current_dir(cwd)
            .env_clear()
            .envs(self.env.iter().map(|(key, value)| (key, value)))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn strace");
        (child, dir)
    }

    /// Run `conspectus <args>` to completion under strace. The command
    /// must succeed, so the scenario exercises what it means to.
    fn run(&self, strace: &Path, label: &str, args: &[&str], cwd: &Path) -> PathBuf {
        let (child, dir) = self.spawn(strace, label, args, cwd);
        let output = child.wait_with_output().expect("wait for strace");
        assert!(
            output.status.success(),
            "`conspectus {}` failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
        dir
    }
}

/// The pid of the traced Conspectus root: the thread whose file starts
/// by running the binary.
fn traced_root(dir: &Path) -> Option<i32> {
    fs::read_dir(dir).ok()?.flatten().find_map(|entry| {
        let text = fs::read_to_string(entry.path()).ok()?;
        let first = text.lines().next()?;
        first
            .starts_with(&format!("execve(\"{BINARY}\""))
            .then(|| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .rsplit('.')
                    .next()?
                    .parse()
                    .ok()
            })?
    })
}

fn wait_for(path: &Path, timeout: Duration) {
    let start = Instant::now();
    while !path.exists() {
        assert!(
            start.elapsed() < timeout,
            "{} never appeared",
            path.display()
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

#[test]
fn the_built_binary_stays_inside_the_capability_manifest() {
    let Some(strace) = strace() else {
        return;
    };
    if !cfg!(debug_assertions) {
        eprintln!("skipping the capability trace: the test world needs a debug build");
        return;
    }
    let manifest = load_manifest();
    let world = World::new();
    let mut report: Vec<String> = Vec::new();
    let mut checked = 0usize;
    let mut record = |scenario: &Scenario, dirs: &[PathBuf], cwd: &Path| {
        let mut events = BTreeSet::new();
        for dir in dirs {
            events.extend(conspectus_events(dir, cwd));
        }
        checked += events.len();
        if std::env::var_os("CONSPECTUS_CAPABILITY_REPORT").is_some() {
            eprintln!("[{}]", scenario.name);
            for event in &events {
                eprintln!("  {event:?}");
            }
        }
        for event in &events {
            if let Some(problem) = check(&manifest, scenario, event) {
                report.push(format!("[{}] {problem}", scenario.name));
            }
        }
    };

    // Building the world is itself the dev scenario.
    let scratch = world.world.to_string_lossy().into_owned();
    let dev = world.run(
        &strace,
        "dev",
        &["dev", "scenario", "materialize", "showcase", &scratch],
        &world.root,
    );
    record(
        &Scenario {
            name: "dev scenario materialize",
            contexts: &["read", "dev"],
            vars: &world.vars(Some(&world.world)),
            pager: None,
        },
        &[dev],
        &world.root,
    );

    let vars = world.vars(None);
    let project = &world.project;

    // Read-only commands: discovery, tables, exports, the TUI.
    let mut reads = vec![
        world.run(&strace, "graph", &["graph", "--format", "json"], project),
        world.run(&strace, "table", &["table", "sessions"], project),
        world.run(&strace, "node", &["columns", "sessions"], project),
    ];
    if cfg!(feature = "snapshot") {
        // The TUI opens on the project's repo row, which previews its
        // history; the second walks onto a session's checkout, which
        // previews its status.
        for (label, keys) in [("tui-repo", ""), ("tui-checkout", "j<Tab>jjjjj")] {
            reads.push(world.run(
                &strace,
                label,
                &[
                    "tui",
                    "--snapshot",
                    "--snapshot-keys",
                    keys,
                    "--snapshot-width",
                    "120",
                ],
                project,
            ));
        }
    }
    record(
        &Scenario {
            name: "read-only commands",
            contexts: &["read"],
            vars: &vars,
            pager: None,
        },
        &reads,
        project,
    );

    // The daemon, with clients asking it for a snapshot and a forge
    // refresh.
    let (serve, serve_dir) = world.spawn(&strace, "serve", &["serve"], project);
    let socket = world.root.join("xdg/run/conspectus/server.sock");
    wait_for(&socket, Duration::from_secs(60));
    let clients = vec![
        world.run(&strace, "status", &["status"], project),
        world.run(
            &strace,
            "refresh",
            &["refresh", "--class", "forge"],
            project,
        ),
        world.run(&strace, "client-table", &["table", "mux"], project),
    ];
    let pid = traced_root(&serve_dir).expect("traced serve pid");
    // SAFETY: `kill` with a pid this test spawned and a valid signal.
    unsafe {
        libc::kill(pid, libc::SIGTERM);
    }
    let output = serve.wait_with_output().expect("wait for serve");
    assert!(
        output.status.success(),
        "`conspectus serve` failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let mut daemon = clients;
    daemon.push(serve_dir);
    record(
        &Scenario {
            name: "daemon and clients",
            contexts: &["read"],
            vars: &vars,
            pager: None,
        },
        &daemon,
        project,
    );

    // Commands the operator runs to change something.
    let cwd = project.to_string_lossy().into_owned();
    let acts = vec![
        world.run(
            &strace,
            "pin-add",
            &[
                "pin",
                "add",
                "--planned",
                "--kind",
                "mux",
                "--name",
                "demo",
                "--set",
                &format!("cwd={cwd}"),
            ],
            project,
        ),
        world.run(
            &strace,
            "pin-edit",
            &["pin", "edit", "demo", "--set", "harness=codex"],
            project,
        ),
        world.run(
            &strace,
            "pin-rename",
            &["pin", "rename", "demo", "demo2"],
            project,
        ),
        world.run(&strace, "pin-rm", &["pin", "rm", "demo2"], project),
        world.run(
            &strace,
            "mux-new-pinned",
            &["mux", "new", "later", "--pin", "--no-launch"],
            project,
        ),
        world.run(
            &strace,
            "mux-new",
            &["mux", "new", "scratch", "--cwd", &cwd, "--no-attach"],
            project,
        ),
        world.run(
            &strace,
            "worktree-new",
            &["worktree", "new", "feat-x", "--repo", &cwd],
            project,
        ),
    ];
    record(
        &Scenario {
            name: "operator actions",
            contexts: &["read", "act"],
            vars: &vars,
            pager: None,
        },
        &acts,
        project,
    );

    // Rebuilds (ADR 0142): a gone mux comes back through tmux, and a
    // gone repo is cloned into a path that doesn't exist.
    let clone_dest = world.root.join("clone-dest");
    let clone_source = world.root.join("clone-source");
    fs::create_dir_all(&clone_source).expect("clone source");
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
            "init",
        ][..],
    ] {
        let status = Command::new("git")
            .args(args)
            .current_dir(&clone_source)
            .status()
            .expect("git");
        assert!(status.success());
    }
    let history_dir = world.root.join("xdg/state/conspectus/history");
    fs::create_dir_all(&history_dir).expect("history dir");
    let history_record = |id: &str, key: &str, recipe: serde_json::Value| {
        serde_json::json!({
            "id": id, "key": key, "first_seen": 100, "last_seen": 200,
            "origin": "observed", "display": {"label": id}, "recipe": recipe,
        })
    };
    let write_history = |file: &str, records: Vec<serde_json::Value>| {
        let body = serde_json::json!({"schema_version": 1, "records": records});
        fs::write(history_dir.join(file), body.to_string()).expect("history file");
    };
    write_history(
        "repos.json",
        vec![history_record(
            "h_clone",
            &format!("repo:{}/.git", clone_dest.display()),
            serde_json::json!({
                "common_dir": format!("{}/.git", clone_dest.display()),
                "remotes": [{"name": "origin", "url": clone_source.display().to_string()}],
                "primary_root": clone_dest.display().to_string(),
            }),
        )],
    );
    write_history(
        "muxes.json",
        vec![history_record(
            "h_mux",
            "mux_session:tmux:rebuilt",
            serde_json::json!({
                "backend": "tmux", "name": "rebuilt",
                "cwd": project.display().to_string(),
                "argv": ["sleep", "600"],
            }),
        )],
    );
    let rebuilds = vec![
        world.run(
            &strace,
            "rebuild-mux",
            &["rebuild", "h_mux", "--no-attach"],
            project,
        ),
        world.run(
            &strace,
            "rebuild-clone",
            &["rebuild", "h_clone", "--no-attach"],
            project,
        ),
    ];
    assert!(clone_dest.join(".git").is_dir(), "the repo was cloned");
    let mut clone_vars = world.vars(None);
    clone_vars
        .0
        .insert("CLONE", vec![clone_dest.to_string_lossy().into_owned()]);
    record(
        &Scenario {
            name: "rebuilds",
            contexts: &["read", "act"],
            vars: &clone_vars,
            pager: None,
        },
        &rebuilds,
        project,
    );

    // Hook installation and removal.
    let hooks = vec![
        world.run(
            &strace,
            "hook-init-claude",
            &["hook", "init", "claude-code"],
            project,
        ),
        world.run(
            &strace,
            "hook-init-codex",
            &["hook", "init", "codex"],
            project,
        ),
        world.run(
            &strace,
            "hook-remove-claude",
            &["hook", "remove", "claude-code"],
            project,
        ),
        world.run(
            &strace,
            "hook-remove-codex",
            &["hook", "remove", "codex"],
            project,
        ),
        world.run(&strace, "hook-init-pi", &["hook", "init", "pi"], project),
        world.run(
            &strace,
            "hook-init-pi-project",
            &["hook", "init", "pi", "--scope", "project"],
            project,
        ),
        world.run(
            &strace,
            "hook-remove-pi",
            &["hook", "remove", "pi"],
            project,
        ),
        world.run(
            &strace,
            "hook-remove-pi-project",
            &["hook", "remove", "pi", "--scope", "project"],
            project,
        ),
    ];
    record(
        &Scenario {
            name: "hook install",
            contexts: &["read", "hook"],
            vars: &vars,
            pager: None,
        },
        &hooks,
        project,
    );

    assert!(checked > 0, "the traces recorded nothing to check");
    report.sort();
    report.dedup();
    assert!(
        report.is_empty(),
        "the built binary did things docs/capabilities.toml doesn't allow (ADR 0117). \
         If a change is intended, add it to the manifest with the ADR that allows it:\n{}",
        report.join("\n")
    );
}
