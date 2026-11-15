use std::os::unix::fs::symlink;

use tempfile::TempDir;

use super::*;
use crate::discovery::harness::dirs::env_of;

/// A scratch world: a fake Nix store, a home, and helpers that put a
/// small executable file (a stand-in binary) where a test needs it.
struct World {
    _temp: TempDir,
    root: PathBuf,
}

impl World {
    fn new() -> Self {
        let temp = TempDir::new().expect("temp");
        let root = fs::canonicalize(temp.path()).expect("canonical temp");
        Self { _temp: temp, root }
    }

    fn path(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }

    fn store(&self) -> PathBuf {
        self.path("nix/store")
    }

    /// An executable file at `relative` holding `content`.
    fn binary(&self, relative: &str, content: &str) -> PathBuf {
        let path = self.path(relative);
        fs::create_dir_all(path.parent().expect("parent")).expect("dir");
        crate::test_program::write_program(path, content)
    }

    /// A symlink at `relative` to `target`.
    fn link(&self, relative: &str, target: &Path) -> PathBuf {
        let path = self.path(relative);
        fs::create_dir_all(path.parent().expect("parent")).expect("dir");
        symlink(target, &path).expect("symlink");
        path
    }

    /// An environment with this world's store and home, and `path`.
    fn env(&self, path: &[&Path]) -> impl Fn(&str) -> Option<OsString> + use<> {
        let joined = std::env::join_paths(path).expect("PATH");
        let store = self.store();
        let home = self.path("home");
        env_of(&[
            ("NIX_STORE_DIR", store.to_str().expect("utf8")),
            ("HOME", home.to_str().expect("utf8")),
            ("PATH", joined.to_str().expect("utf8")),
        ])
    }
}

#[test]
fn a_binary_that_stays_is_installed_as_it_is() {
    let world = World::new();
    let exe = world.binary("usr/local/bin/conspectus", "build-1");
    let choice = choose(&exe, &world.env(&[]));
    assert_eq!(choice.why, Why::Running);
    assert_eq!(choice.program, exe);
    assert_eq!(choice.warning("conspectus hook init claude-code"), None);
    assert_eq!(
        choice.command("claude-code"),
        format!("{} hook write claude-code", exe.display())
    );
}

#[test]
fn a_store_binary_prefers_the_path_name_that_links_to_it() {
    let world = World::new();
    let exe = world.binary("nix/store/abc-conspectus/bin/conspectus", "build-1");
    let profile_bin = world.path("home/.nix-profile/bin");
    let on_path = world.link("home/.nix-profile/bin/conspectus", &exe);
    let choice = choose(&exe, &world.env(&[&profile_bin]));
    assert_eq!(choice.why, Why::OnPath);
    assert_eq!(choice.program, on_path);
    assert!(
        choice.describe().contains("on PATH"),
        "{}",
        choice.describe()
    );
}

#[test]
fn a_path_name_that_runs_another_build_is_passed_over_for_a_profile_link() {
    let world = World::new();
    let exe = world.binary("nix/store/new-conspectus/bin/conspectus", "build-2");
    let other = world.binary("opt/old/bin/conspectus", "build-1");
    let profile = world.link("home/.nix-profile/bin/conspectus", &exe);
    let choice = choose(&exe, &world.env(&[other.parent().expect("dir")]));
    assert_eq!(choice.why, Why::Profile);
    assert_eq!(choice.program, profile);
    assert!(
        choice.describe().contains("Nix profile"),
        "{}",
        choice.describe()
    );
}

#[test]
fn a_store_binary_with_no_path_that_stays_falls_back_and_says_why() {
    let world = World::new();
    let exe = world.binary("nix/store/abc-conspectus/bin/conspectus", "build-1");
    // A PATH entry inside the store (a `nix shell`) goes the same way.
    let shell_bin = world.path("nix/store/def-env/bin");
    world.link("nix/store/def-env/bin/conspectus", &exe);
    let elsewhere = world.binary("opt/old/bin/conspectus", "build-0");
    let choice = choose(
        &exe,
        &world.env(&[&shell_bin, elsewhere.parent().expect("dir")]),
    );
    assert_eq!(choice.why, Why::Fallback(Fragile::NixStore));
    assert_eq!(choice.program, exe);
    let warning = choice
        .warning("csp hook init codex")
        .expect("a fallback warns");
    assert!(warning.contains("Nix store"), "{warning}");
    assert!(warning.contains("`csp hook init codex`"), "{warning}");
}

#[test]
fn a_copy_with_the_same_content_counts_as_the_same_build() {
    let world = World::new();
    let exe = world.binary("nix/store/abc-conspectus/bin/conspectus", "build-1");
    let copy = world.binary("home/.cargo/bin/conspectus", "build-1");
    let choice = choose(&exe, &world.env(&[copy.parent().expect("dir")]));
    assert_eq!(choice.why, Why::OnPath);
    assert_eq!(choice.program, copy);
}

#[test]
fn a_cargo_build_is_fragile_until_a_path_name_links_to_it() {
    let world = World::new();
    let exe = world.binary("src/conspectus/target/debug/conspectus", "dev");
    let choice = choose(&exe, &world.env(&[]));
    assert_eq!(choice.why, Why::Fallback(Fragile::Build));
    assert!(
        choice
            .warning("conspectus hook init claude-code")
            .expect("warns")
            .contains("cargo build directory")
    );

    // A custom target directory is known by its `CACHEDIR.TAG`.
    let custom = world.binary("cache/cargo-target/x86_64/opt/conspectus", "dev");
    fs::write(world.path("cache/cargo-target/CACHEDIR.TAG"), "tag").expect("tag");
    assert_eq!(fragile(&custom, &world.env(&[])), Some(Fragile::Build));

    let bin = world.path("home/bin");
    let link = world.link("home/bin/conspectus", &exe);
    let choice = choose(&exe, &world.env(&[&bin]));
    assert_eq!(choice.why, Why::OnPath);
    assert_eq!(choice.program, link);
}

#[test]
fn the_default_store_is_nix_store() {
    let env = env_of(&[]);
    assert_eq!(
        fragile(Path::new("/nix/store/abc-conspectus/bin/conspectus"), &env),
        Some(Fragile::NixStore)
    );
    assert_eq!(fragile(Path::new("/usr/bin/conspectus"), &env), None);
    assert_eq!(fragile(Path::new("conspectus"), &env), None);
}

#[test]
fn an_installed_program_is_found_gone_or_off_this_path() {
    let world = World::new();
    let exe = world.binary("opt/bin/conspectus", "build-1");
    let env = world.env(&[exe.parent().expect("dir")]);
    assert_eq!(standing(exe.to_str().expect("utf8"), &env), Program::Found);
    assert_eq!(
        standing(
            world
                .path("nix/store/gone/bin/conspectus")
                .to_str()
                .expect("utf8"),
            &env
        ),
        Program::Gone
    );
    assert_eq!(standing("conspectus", &env), Program::Found);
    assert_eq!(standing("conspectus", &world.env(&[])), Program::NotOnPath);
}

#[test]
fn the_program_is_a_command_lines_first_word() {
    assert_eq!(
        program_of("/nix/store/abc/bin/conspectus hook write claude-code").as_deref(),
        Some("/nix/store/abc/bin/conspectus")
    );
    assert_eq!(
        program_of("'/home/me/my tools/conspectus' hook write codex").as_deref(),
        Some("/home/me/my tools/conspectus")
    );
    assert_eq!(
        program_of(r#""/opt/a \"b\"/conspectus" hook write codex"#).as_deref(),
        Some(r#"/opt/a "b"/conspectus"#)
    );
    assert_eq!(
        program_of(r"/opt/a\ b/conspectus hook write codex").as_deref(),
        Some("/opt/a b/conspectus")
    );
    assert_eq!(
        program_of("RUST_LOG=warn env FOO=1 conspectus hook write codex").as_deref(),
        Some("conspectus")
    );
    assert_eq!(program_of("   ").as_deref(), None);
    assert_eq!(program_of("").as_deref(), None);
}

#[test]
fn a_program_path_is_quoted_only_when_it_needs_it() {
    assert_eq!(
        shell_quote("/nix/store/a-b_c.d/bin/x"),
        "/nix/store/a-b_c.d/bin/x"
    );
    assert_eq!(shell_quote("/home/me/my tools/x"), "'/home/me/my tools/x'");
    assert_eq!(shell_quote("/it's/x"), r"'/it'\''s/x'");
    for program in ["/home/me/my tools/x", "/it's/x", r#"/a "b"/x"#] {
        let line = format!("{} hook write codex", shell_quote(program));
        assert_eq!(program_of(&line).as_deref(), Some(program), "{line}");
    }
}
