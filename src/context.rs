use crate::error::SenditError;
use crate::gh::{self, ExistingPr};
use crate::git::{self, CommitEntry, DiffStat};

/// Everything `sendit` learns about the branch before drafting a PR.
#[derive(Debug, Clone)]
pub struct PrContext {
    pub branch: String,
    pub trunk: String,
    pub merge_base: String,
    pub has_upstream: bool,
    pub commits_ahead: u32,
    pub commit_log: Vec<CommitEntry>,
    pub diffstat: DiffStat,
    pub existing_pr: Option<ExistingPr>,
}

/// Decide the trunk/base branch from the available signals.
///
/// Precedence: an explicit `--base` override, then `gh`'s reported default
/// branch, then the local `origin/HEAD` symref, then a local `main`/`master`.
pub fn resolve_trunk(
    base_override: Option<&str>,
    gh_default: Option<&str>,
    origin_head: Option<&str>,
    main_exists: bool,
    master_exists: bool,
) -> Result<String, SenditError> {
    if let Some(base) = base_override {
        return Ok(base.to_string());
    }
    if let Some(name) = gh_default {
        return Ok(name.to_string());
    }
    if let Some(name) = origin_head {
        return Ok(name.to_string());
    }
    if main_exists {
        return Ok("main".to_string());
    }
    if master_exists {
        return Ok("master".to_string());
    }
    Err(SenditError::NoTrunk)
}

/// Run every preflight check and gather the branch context, or return the first
/// failing check as an error.
pub fn preflight(base_override: Option<&str>) -> Result<PrContext, SenditError> {
    if !git::is_inside_repo()? {
        return Err(SenditError::NotARepo);
    }
    if !gh::auth_ok()? {
        return Err(SenditError::NotAuthenticated);
    }

    let branch = git::current_branch()?;

    let gh_default = gh::default_branch();
    let origin_head = git::origin_head();
    let trunk = resolve_trunk(
        base_override,
        gh_default.as_deref(),
        origin_head.as_deref(),
        git::local_branch_exists("main"),
        git::local_branch_exists("master"),
    )?;

    if branch == trunk {
        return Err(SenditError::OnTrunk(trunk));
    }

    let merge_base = git::merge_base(&trunk)?;
    let commits_ahead = git::commits_ahead(&merge_base)?;
    if commits_ahead == 0 {
        return Err(SenditError::NothingToSend { trunk });
    }

    let has_upstream = git::has_upstream();
    let commit_log = git::commit_log(&merge_base)?;
    let diffstat = git::diffstat(&merge_base)?;
    let existing_pr = gh::find_existing_pr(&branch)?;

    Ok(PrContext {
        branch,
        trunk,
        merge_base,
        has_upstream,
        commits_ahead,
        commit_log,
        diffstat,
        existing_pr,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn override_wins() {
        let t = resolve_trunk(Some("develop"), Some("main"), Some("master"), true, true).unwrap();
        assert_eq!(t, "develop");
    }

    #[test]
    fn gh_default_used() {
        let t = resolve_trunk(None, Some("main"), Some("master"), false, false).unwrap();
        assert_eq!(t, "main");
    }

    #[test]
    fn origin_head_fallback() {
        let t = resolve_trunk(None, None, Some("master"), false, false).unwrap();
        assert_eq!(t, "master");
    }

    #[test]
    fn local_main_fallback() {
        let t = resolve_trunk(None, None, None, true, false).unwrap();
        assert_eq!(t, "main");
    }

    #[test]
    fn local_master_fallback() {
        let t = resolve_trunk(None, None, None, false, true).unwrap();
        assert_eq!(t, "master");
    }

    #[test]
    fn no_trunk_errors() {
        let err = resolve_trunk(None, None, None, false, false);
        assert!(matches!(err, Err(SenditError::NoTrunk)));
    }
}
