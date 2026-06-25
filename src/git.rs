use std::process::{Command, Output};

use crate::error::SenditError;

/// A single commit between the merge-base and HEAD.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitEntry {
    pub hash: String,
    pub subject: String,
}

/// Aggregate diff statistics for the branch versus its merge-base.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffStat {
    pub files: u32,
    pub insertions: u32,
    pub deletions: u32,
    /// The `git diff --stat` display block (per-file bars + summary line).
    pub raw: String,
}

/// Build a `git` Command preconfigured with `-c core.quotepath=false` so non-ASCII
/// path bytes are emitted as raw UTF-8 instead of `\NNN` octal escapes.
pub fn git_cmd() -> Command {
    let mut cmd = Command::new("git");
    cmd.args(["-c", "core.quotepath=false"]);
    cmd
}

/// Run a `git` command, mapping a missing binary to `GitNotInstalled`.
fn git_output(args: &[&str]) -> Result<Output, SenditError> {
    git_cmd().args(args).output().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            SenditError::GitNotInstalled
        } else {
            SenditError::Io(e)
        }
    })
}

/// Run a `git` command and return its stdout, requiring success.
///
/// Maps "not a git repository" (exit 128) to `NotARepo` and any other failure
/// to `GitFailed` with the trimmed stderr.
fn run_git(args: &[&str]) -> Result<String, SenditError> {
    let output = git_output(args)?;
    if output.status.code() == Some(128) {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("not a git repository") {
            return Err(SenditError::NotARepo);
        }
        return Err(SenditError::GitFailed(stderr.trim().to_string()));
    }
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(SenditError::GitFailed(stderr.trim().to_string()));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Whether the current directory is inside a git work tree.
pub fn is_inside_repo() -> Result<bool, SenditError> {
    let output = git_output(&["rev-parse", "--is-inside-work-tree"])?;
    Ok(output.status.success() && String::from_utf8_lossy(&output.stdout).trim() == "true")
}

/// The current branch name, or `DetachedHead` when HEAD isn't on a branch.
///
/// Assumes the repository check has already passed (see [`is_inside_repo`]).
pub fn current_branch() -> Result<String, SenditError> {
    let output = git_output(&["symbolic-ref", "--quiet", "--short", "HEAD"])?;
    if output.status.success() {
        let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !branch.is_empty() {
            return Ok(branch);
        }
    }
    Err(SenditError::DetachedHead)
}

/// The short name behind `refs/remotes/origin/HEAD` (e.g. `main`), if set.
pub fn origin_head() -> Option<String> {
    let output = git_output(&["symbolic-ref", "--short", "refs/remotes/origin/HEAD"]).ok()?;
    if !output.status.success() {
        return None;
    }
    let full = String::from_utf8_lossy(&output.stdout).trim().to_string();
    // Strip the leading `origin/` so callers get a bare branch name.
    full.strip_prefix("origin/")
        .map(str::to_string)
        .filter(|s| !s.is_empty())
        .or(Some(full).filter(|s| !s.is_empty()))
}

/// Whether a local branch with the given name exists.
pub fn local_branch_exists(name: &str) -> bool {
    git_output(&[
        "rev-parse",
        "--verify",
        "--quiet",
        &format!("refs/heads/{name}"),
    ])
    .map(|o| o.status.success())
    .unwrap_or(false)
}

/// The merge-base commit between HEAD and `trunk`.
pub fn merge_base(trunk: &str) -> Result<String, SenditError> {
    Ok(run_git(&["merge-base", "HEAD", trunk])?.trim().to_string())
}

/// Number of commits on HEAD that are not reachable from `base`.
pub fn commits_ahead(base: &str) -> Result<u32, SenditError> {
    let out = run_git(&["rev-list", "--count", &format!("{base}..HEAD")])?;
    out.trim()
        .parse::<u32>()
        .map_err(|e| SenditError::GitFailed(format!("unexpected rev-list output: {e}")))
}

/// Whether the current branch has a configured upstream tracking branch.
pub fn has_upstream() -> bool {
    git_output(&["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"])
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Commit log (hash + subject) for `base..HEAD`, newest first.
pub fn commit_log(base: &str) -> Result<Vec<CommitEntry>, SenditError> {
    let raw = run_git(&["log", "--format=%h %s", &format!("{base}..HEAD")])?;
    Ok(parse_commit_log(&raw))
}

/// Diff statistics for `base..HEAD` (display block + parsed counts).
pub fn diffstat(base: &str) -> Result<DiffStat, SenditError> {
    let range = format!("{base}..HEAD");
    let raw = run_git(&["diff", "--stat", &range])?;
    let short = run_git(&["diff", "--shortstat", &range])?;
    let (files, insertions, deletions) = parse_shortstat(&short);
    Ok(DiffStat {
        files,
        insertions,
        deletions,
        raw: raw.trim_end().to_string(),
    })
}

/// Push the current branch, setting upstream when none is configured.
///
/// Never force-pushes and never targets a branch other than the current one.
pub fn push(branch: &str, set_upstream: bool) -> Result<(), SenditError> {
    let output = if set_upstream {
        git_output(&["push", "-u", "origin", branch])?
    } else {
        git_output(&["push"])?
    };
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(SenditError::GitFailed(stderr.trim().to_string()));
    }
    Ok(())
}

/// Parse `git log --format=%h %s` output into commit entries.
pub fn parse_commit_log(raw: &str) -> Vec<CommitEntry> {
    raw.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|line| match line.split_once(' ') {
            Some((hash, subject)) => CommitEntry {
                hash: hash.to_string(),
                subject: subject.to_string(),
            },
            None => CommitEntry {
                hash: line.to_string(),
                subject: String::new(),
            },
        })
        .collect()
}

/// Parse a `git diff --shortstat` line into `(files, insertions, deletions)`.
///
/// Handles all the partial forms git emits, e.g.
/// ` 3 files changed, 42 insertions(+), 7 deletions(-)`,
/// ` 1 file changed, 5 insertions(+)`, ` 2 files changed, 3 deletions(-)`.
pub fn parse_shortstat(line: &str) -> (u32, u32, u32) {
    let (mut files, mut insertions, mut deletions) = (0u32, 0u32, 0u32);
    for part in line.split(',') {
        let part = part.trim();
        let num = part
            .split_whitespace()
            .next()
            .and_then(|n| n.parse::<u32>().ok());
        if let Some(n) = num {
            if part.contains("file") {
                files = n;
            } else if part.contains("insertion") {
                insertions = n;
            } else if part.contains("deletion") {
                deletions = n;
            }
        }
    }
    (files, insertions, deletions)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_commit_log_basic() {
        let raw = "a1b2c3d Add the widget loader\nd4e5f6a Wire widgets in\n";
        let log = parse_commit_log(raw);
        assert_eq!(log.len(), 2);
        assert_eq!(log[0].hash, "a1b2c3d");
        assert_eq!(log[0].subject, "Add the widget loader");
        assert_eq!(log[1].hash, "d4e5f6a");
        assert_eq!(log[1].subject, "Wire widgets in");
    }

    #[test]
    fn parse_commit_log_skips_blank_and_handles_no_subject() {
        let log = parse_commit_log("\nabc123\n\n");
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].hash, "abc123");
        assert_eq!(log[0].subject, "");
    }

    #[test]
    fn parse_shortstat_full() {
        let (f, i, d) = parse_shortstat(" 3 files changed, 42 insertions(+), 7 deletions(-)");
        assert_eq!((f, i, d), (3, 42, 7));
    }

    #[test]
    fn parse_shortstat_insertions_only() {
        let (f, i, d) = parse_shortstat(" 1 file changed, 5 insertions(+)");
        assert_eq!((f, i, d), (1, 5, 0));
    }

    #[test]
    fn parse_shortstat_deletions_only() {
        let (f, i, d) = parse_shortstat(" 2 files changed, 3 deletions(-)");
        assert_eq!((f, i, d), (2, 0, 3));
    }

    #[test]
    fn parse_shortstat_empty() {
        assert_eq!(parse_shortstat(""), (0, 0, 0));
    }
}
