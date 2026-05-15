//! Read-only git discovery probes.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitProbe {
    git_bin: PathBuf,
}

impl Default for GitProbe {
    fn default() -> Self {
        Self {
            git_bin: PathBuf::from("git"),
        }
    }
}

impl GitProbe {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn probe(&self, root: impl AsRef<Path>) -> Result<Option<GitProbeResult>> {
        let root = root.as_ref();

        if !self.is_inside_work_tree(root)? {
            return Ok(None);
        }

        let common_dir = self.required(
            root,
            &["rev-parse", "--path-format=absolute", "--git-common-dir"],
        )?;
        let worktree_root = self.required(root, &["rev-parse", "--show-toplevel"])?;
        let git_dir = self.required(root, &["rev-parse", "--path-format=absolute", "--git-dir"])?;
        let branch_ref = self.optional(root, &["symbolic-ref", "--quiet", "HEAD"])?;
        let upstream = self.optional(
            root,
            &[
                "rev-parse",
                "--abbrev-ref",
                "--symbolic-full-name",
                "@{upstream}",
            ],
        )?;
        let remotes = self.remotes(root)?;

        Ok(Some(GitProbeResult {
            common_dir: PathBuf::from(common_dir),
            worktree_root: PathBuf::from(worktree_root),
            git_dir: PathBuf::from(git_dir),
            branch_ref,
            upstream,
            remotes,
        }))
    }

    fn is_inside_work_tree(&self, root: &Path) -> Result<bool> {
        match self.optional(root, &["rev-parse", "--is-inside-work-tree"])? {
            Some(value) => Ok(value == "true"),
            None => Ok(false),
        }
    }

    fn remotes(&self, root: &Path) -> Result<Vec<GitRemote>> {
        let Some(names) = self.optional(root, &["remote"])? else {
            return Ok(Vec::new());
        };
        let mut remotes = Vec::new();

        for name in names.lines().filter(|line| !line.trim().is_empty()) {
            let url = self
                .optional(root, &["remote", "get-url", name])?
                .unwrap_or_default();
            remotes.push(GitRemote {
                name: name.to_string(),
                url,
            });
        }

        remotes.sort();
        Ok(remotes)
    }

    fn required(&self, root: &Path, args: &[&str]) -> Result<String> {
        self.optional(root, args)?
            .with_context(|| format!("git command failed: git {}", args.join(" ")))
    }

    fn optional(&self, root: &Path, args: &[&str]) -> Result<Option<String>> {
        let output = Command::new(&self.git_bin)
            .args(args)
            .current_dir(root)
            .output()
            .with_context(|| format!("failed to run git {}", args.join(" ")))?;

        if output.status.success() {
            return Ok(Some(trim_output(output.stdout)));
        }

        if is_expected_absence(args, output.status.code()) {
            return Ok(None);
        }

        bail!(
            "git {} failed with status {}: {}",
            args.join(" "),
            output.status,
            trim_output(output.stderr)
        );
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitProbeResult {
    pub common_dir: PathBuf,
    pub worktree_root: PathBuf,
    pub git_dir: PathBuf,
    pub branch_ref: Option<String>,
    pub upstream: Option<String>,
    pub remotes: Vec<GitRemote>,
}

impl GitProbeResult {
    pub fn is_linked_worktree(&self) -> bool {
        self.common_dir != self.git_dir
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct GitRemote {
    pub name: String,
    pub url: String,
}

fn trim_output(bytes: Vec<u8>) -> String {
    String::from_utf8_lossy(&bytes).trim().to_string()
}

fn is_expected_absence(args: &[&str], code: Option<i32>) -> bool {
    matches!(
        (args, code),
        (["rev-parse", "--is-inside-work-tree"], Some(128))
            | (["symbolic-ref", "--quiet", "HEAD"], Some(1))
            | (
                [
                    "rev-parse",
                    "--abbrev-ref",
                    "--symbolic-full-name",
                    "@{upstream}"
                ],
                Some(128)
            )
            | (["remote"], Some(_))
    )
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    #[test]
    fn probe_returns_none_outside_git_repo() {
        let temp = TempDir::new().expect("temp dir");

        let result = GitProbe::new().probe(temp.path()).expect("probe succeeds");

        assert!(result.is_none());
    }

    #[test]
    fn probe_reads_plain_repo_identity_branch_remote_and_upstream() {
        let fixture = GitFixture::init();
        fixture.git(&["checkout", "-b", "feature"]);
        fixture.git(&["remote", "add", "origin", "git@example.com:owner/repo.git"]);
        fixture.git(&["update-ref", "refs/remotes/origin/feature", "HEAD"]);
        fixture.git(&["branch", "--set-upstream-to", "origin/feature", "feature"]);

        let result = GitProbe::new()
            .probe(fixture.root())
            .expect("probe succeeds")
            .expect("git repo discovered");

        assert_eq!(
            result.worktree_root,
            fixture.root().canonicalize().expect("canonical root")
        );
        assert_eq!(result.branch_ref.as_deref(), Some("refs/heads/feature"));
        assert_eq!(result.upstream.as_deref(), Some("origin/feature"));
        assert_eq!(
            result.remotes,
            vec![GitRemote {
                name: "origin".to_string(),
                url: "git@example.com:owner/repo.git".to_string(),
            }]
        );
        assert!(!result.is_linked_worktree());
    }

    #[test]
    fn probe_allows_detached_head_without_branch() {
        let fixture = GitFixture::init();
        let commit = fixture.git_stdout(&["rev-parse", "HEAD"]);
        fixture.git(&["checkout", "--detach", &commit]);

        let result = GitProbe::new()
            .probe(fixture.root())
            .expect("probe succeeds")
            .expect("git repo discovered");

        assert_eq!(result.branch_ref, None);
        assert_eq!(result.upstream, None);
    }

    #[test]
    fn probe_reads_linked_worktree_metadata() {
        let fixture = GitFixture::init();
        let linked = fixture.parent().join("linked");
        fixture.git(&["worktree", "add", "-b", "linked-branch", path_str(&linked)]);

        let main = GitProbe::new()
            .probe(fixture.root())
            .expect("main probe succeeds")
            .expect("main repo discovered");
        let linked_result = GitProbe::new()
            .probe(&linked)
            .expect("linked probe succeeds")
            .expect("linked worktree discovered");

        assert_eq!(linked_result.common_dir, main.common_dir);
        assert_ne!(linked_result.git_dir, main.git_dir);
        assert_eq!(
            linked_result.branch_ref.as_deref(),
            Some("refs/heads/linked-branch")
        );
        assert!(linked_result.is_linked_worktree());
    }

    struct GitFixture {
        temp: TempDir,
        root: PathBuf,
    }

    impl GitFixture {
        fn init() -> Self {
            let temp = TempDir::new().expect("temp dir");
            let root = temp.path().join("repo");
            fs::create_dir(&root).expect("create repo dir");
            let fixture = Self { temp, root };
            fixture.git(&["init", "--initial-branch", "main"]);
            fixture.git(&["config", "user.name", "Conspectus Test"]);
            fixture.git(&["config", "user.email", "conspectus@example.invalid"]);
            fs::write(fixture.root.join("README.md"), "fixture\n").expect("write fixture file");
            fixture.git(&["add", "README.md"]);
            fixture.git(&["commit", "-m", "initial"]);
            fixture
        }

        fn root(&self) -> &Path {
            &self.root
        }

        fn parent(&self) -> &Path {
            self.temp.path()
        }

        fn git(&self, args: &[&str]) {
            let output = Command::new("git")
                .args(args)
                .current_dir(&self.root)
                .output()
                .expect("run git command");

            assert!(
                output.status.success(),
                "git {} failed: {}",
                args.join(" "),
                trim_output(output.stderr)
            );
        }

        fn git_stdout(&self, args: &[&str]) -> String {
            let output = Command::new("git")
                .args(args)
                .current_dir(&self.root)
                .output()
                .expect("run git command");

            assert!(
                output.status.success(),
                "git {} failed: {}",
                args.join(" "),
                trim_output(output.stderr)
            );
            trim_output(output.stdout)
        }
    }

    fn path_str(path: &Path) -> &str {
        path.to_str().expect("utf8 path")
    }
}
