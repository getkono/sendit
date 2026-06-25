use std::ffi::OsStr;
use std::process::{Command, Output};

use serde::Deserialize;

use crate::error::SenditError;

/// An open PR already associated with the current branch.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ExistingPr {
    pub number: u64,
    pub url: String,
    pub state: String,
    #[serde(rename = "isDraft")]
    pub is_draft: bool,
}

#[derive(Deserialize)]
struct DefaultBranchRef {
    name: String,
}

#[derive(Deserialize)]
struct RepoView {
    #[serde(rename = "defaultBranchRef")]
    default_branch_ref: Option<DefaultBranchRef>,
}

/// Run a `gh` command, mapping a missing binary to `GhNotInstalled`.
fn gh_output<I, S>(args: I) -> Result<Output, SenditError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    Command::new("gh").args(args).output().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            SenditError::GhNotInstalled
        } else {
            SenditError::Io(e)
        }
    })
}

/// Run a `gh` command, returning its stdout and requiring success.
pub fn run_gh<I, S>(args: I) -> Result<String, SenditError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = gh_output(args)?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(SenditError::GhFailed(stderr.trim().to_string()));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Whether `gh` is authenticated (`gh auth status` exits zero).
pub fn auth_ok() -> Result<bool, SenditError> {
    Ok(gh_output(["auth", "status"])?.status.success())
}

/// The repository's default branch via `gh repo view`, or `None` on any failure
/// (kept non-fatal so trunk detection can fall back to local git state offline).
pub fn default_branch() -> Option<String> {
    let output = gh_output(["repo", "view", "--json", "defaultBranchRef"]).ok()?;
    if !output.status.success() {
        return None;
    }
    let view: RepoView = serde_json::from_slice(&output.stdout).ok()?;
    view.default_branch_ref.map(|r| r.name)
}

/// Find an open PR whose head is `branch`, if any.
pub fn find_existing_pr(branch: &str) -> Result<Option<ExistingPr>, SenditError> {
    let stdout = run_gh([
        "pr",
        "list",
        "--head",
        branch,
        "--state",
        "open",
        "--json",
        "number,url,state,isDraft",
    ])?;
    let prs: Vec<ExistingPr> =
        serde_json::from_str(&stdout).map_err(|e| SenditError::GhParse(format!("pr list: {e}")))?;
    Ok(prs.into_iter().next())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_existing_pr() {
        let json = r#"[{"number":42,"url":"https://github.com/o/r/pull/42","state":"OPEN","isDraft":true}]"#;
        let prs: Vec<ExistingPr> = serde_json::from_str(json).unwrap();
        assert_eq!(prs.len(), 1);
        assert_eq!(prs[0].number, 42);
        assert_eq!(prs[0].url, "https://github.com/o/r/pull/42");
        assert_eq!(prs[0].state, "OPEN");
        assert!(prs[0].is_draft);
    }

    #[test]
    fn deserialize_empty_pr_list() {
        let prs: Vec<ExistingPr> = serde_json::from_str("[]").unwrap();
        assert!(prs.is_empty());
    }

    #[test]
    fn deserialize_repo_view() {
        let view: RepoView =
            serde_json::from_str(r#"{"defaultBranchRef":{"name":"main"}}"#).unwrap();
        assert_eq!(view.default_branch_ref.unwrap().name, "main");
    }

    #[test]
    fn deserialize_repo_view_null_default() {
        let view: RepoView = serde_json::from_str(r#"{"defaultBranchRef":null}"#).unwrap();
        assert!(view.default_branch_ref.is_none());
    }
}
