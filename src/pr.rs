use crate::context::PrContext;
use crate::error::SenditError;
use crate::gh;

/// The human-authored content of a PR.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrSpec {
    pub title: String,
    pub body: String,
    pub draft: bool,
}

/// Whether to open a new PR or update an existing one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrAction {
    Create,
    Update { number: u64 },
}

/// The result of a create/update operation.
#[derive(Debug, Clone)]
pub struct PrOutcome {
    pub url: String,
    pub number: Option<u64>,
    pub draft: bool,
    pub action: PrAction,
}

/// The create-vs-update decision resolved from the CLI flags and any existing
/// PR, without performing any I/O.
///
/// `Conflict` means an open PR already exists for the branch and no flag forced
/// the choice; the caller must decide how to proceed (prompt interactively,
/// default to one, or error). This lets embedders reuse the decision table
/// without inheriting `sendit`'s terminal prompt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionChoice {
    /// Open a new PR.
    Create,
    /// Update the existing PR with this number.
    Update(u64),
    /// An open PR exists and the choice was not forced; the caller decides.
    Conflict(gh::ExistingPr),
}

/// Resolve an [`ActionChoice`] from the existing PR (if any) and the `--update`
/// / `--new` / `--yes` flags, with no prompting or other I/O.
///
/// Precedence: no existing PR → [`ActionChoice::Create`]; `--update` →
/// [`ActionChoice::Update`]; `--new` → [`ActionChoice::Create`]; `--yes` (the
/// idempotent default for scripting) → [`ActionChoice::Update`]; otherwise
/// [`ActionChoice::Conflict`].
pub fn resolve_action(
    existing: Option<&gh::ExistingPr>,
    update: bool,
    new: bool,
    yes: bool,
) -> ActionChoice {
    let Some(existing) = existing else {
        return ActionChoice::Create;
    };
    if update {
        ActionChoice::Update(existing.number)
    } else if new {
        ActionChoice::Create
    } else if yes {
        // Idempotent default for scripting: update rather than spawn duplicates.
        ActionChoice::Update(existing.number)
    } else {
        ActionChoice::Conflict(existing.clone())
    }
}

/// Build the `gh pr create` argument list.
pub fn build_create_args(ctx: &PrContext, spec: &PrSpec) -> Vec<String> {
    let mut args = vec![
        "pr".to_string(),
        "create".to_string(),
        "--base".to_string(),
        ctx.trunk.clone(),
        "--head".to_string(),
        ctx.branch.clone(),
        "--title".to_string(),
        spec.title.clone(),
        "--body".to_string(),
        spec.body.clone(),
    ];
    if spec.draft {
        args.push("--draft".to_string());
    }
    args
}

/// Build the `gh pr edit <number>` argument list.
pub fn build_edit_args(number: u64, spec: &PrSpec) -> Vec<String> {
    vec![
        "pr".to_string(),
        "edit".to_string(),
        number.to_string(),
        "--title".to_string(),
        spec.title.clone(),
        "--body".to_string(),
        spec.body.clone(),
    ]
}

/// Create or update the PR per `spec` and `action`, returning the outcome.
///
/// Note: `gh pr edit` cannot toggle draft/ready state, so on update the reported
/// draft status reflects the existing PR (use `gh pr ready` to flip it).
pub fn create_or_update_pr(
    ctx: &PrContext,
    spec: &PrSpec,
    action: PrAction,
) -> Result<PrOutcome, SenditError> {
    match action {
        PrAction::Create => {
            let stdout = gh::run_gh(build_create_args(ctx, spec))?;
            let url = parse_pr_url(&stdout);
            let number = parse_pr_number(&url);
            Ok(PrOutcome {
                url,
                number,
                draft: spec.draft,
                action,
            })
        }
        PrAction::Update { number } => {
            let stdout = gh::run_gh(build_edit_args(number, spec))?;
            let url = parse_pr_url(&stdout);
            let draft = ctx
                .existing_pr
                .as_ref()
                .map(|p| p.is_draft)
                .unwrap_or(spec.draft);
            Ok(PrOutcome {
                url: if url.is_empty() {
                    ctx.existing_pr
                        .as_ref()
                        .map(|p| p.url.clone())
                        .unwrap_or_default()
                } else {
                    url
                },
                number: Some(number),
                draft,
                action,
            })
        }
    }
}

/// Extract the PR URL from `gh` stdout (the last line beginning with `http`).
pub fn parse_pr_url(stdout: &str) -> String {
    stdout
        .lines()
        .rev()
        .map(str::trim)
        .find(|l| l.starts_with("http"))
        .unwrap_or("")
        .to_string()
}

/// Extract the trailing PR number from a `.../pull/<n>` URL.
pub fn parse_pr_number(url: &str) -> Option<u64> {
    url.rsplit('/').next().and_then(|s| s.parse::<u64>().ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::DiffStat;

    fn ctx() -> PrContext {
        PrContext {
            branch: "feat".to_string(),
            trunk: "master".to_string(),
            merge_base: "abc".to_string(),
            has_upstream: true,
            commits_ahead: 1,
            commit_log: vec![],
            diffstat: DiffStat {
                files: 0,
                insertions: 0,
                deletions: 0,
                raw: String::new(),
            },
            existing_pr: None,
        }
    }

    #[test]
    fn create_args_ready() {
        let spec = PrSpec {
            title: "T".to_string(),
            body: "B".to_string(),
            draft: false,
        };
        assert_eq!(
            build_create_args(&ctx(), &spec),
            vec![
                "pr", "create", "--base", "master", "--head", "feat", "--title", "T", "--body", "B"
            ]
        );
    }

    #[test]
    fn create_args_draft() {
        let spec = PrSpec {
            title: "T".to_string(),
            body: "B".to_string(),
            draft: true,
        };
        let args = build_create_args(&ctx(), &spec);
        assert_eq!(args.last().unwrap(), "--draft");
    }

    #[test]
    fn create_args_empty_body_still_emitted() {
        let spec = PrSpec {
            title: "T".to_string(),
            body: String::new(),
            draft: false,
        };
        let args = build_create_args(&ctx(), &spec);
        let i = args.iter().position(|a| a == "--body").unwrap();
        assert_eq!(args[i + 1], "");
    }

    #[test]
    fn edit_args_basic() {
        let spec = PrSpec {
            title: "T".to_string(),
            body: "B".to_string(),
            draft: false,
        };
        assert_eq!(
            build_edit_args(7, &spec),
            vec!["pr", "edit", "7", "--title", "T", "--body", "B"]
        );
    }

    #[test]
    fn url_parsing() {
        let out = "Creating pull request for feat into master\nhttps://github.com/o/r/pull/12\n";
        assert_eq!(parse_pr_url(out), "https://github.com/o/r/pull/12");
        assert_eq!(parse_pr_number("https://github.com/o/r/pull/12"), Some(12));
    }

    #[test]
    fn url_parsing_none() {
        assert_eq!(parse_pr_url("nothing here\n"), "");
        assert_eq!(parse_pr_number(""), None);
    }

    fn existing() -> gh::ExistingPr {
        gh::ExistingPr {
            number: 42,
            url: "https://github.com/o/r/pull/42".to_string(),
            state: "OPEN".to_string(),
            is_draft: false,
        }
    }

    #[test]
    fn resolve_action_no_existing_is_create() {
        assert_eq!(
            resolve_action(None, false, false, false),
            ActionChoice::Create
        );
        // Flags are irrelevant with no existing PR.
        assert_eq!(
            resolve_action(None, true, false, true),
            ActionChoice::Create
        );
    }

    #[test]
    fn resolve_action_update_flag_wins() {
        assert_eq!(
            resolve_action(Some(&existing()), true, false, false),
            ActionChoice::Update(42)
        );
    }

    #[test]
    fn resolve_action_new_flag_forces_create() {
        assert_eq!(
            resolve_action(Some(&existing()), false, true, false),
            ActionChoice::Create
        );
        // `--new` also wins over the idempotent `--yes` default.
        assert_eq!(
            resolve_action(Some(&existing()), false, true, true),
            ActionChoice::Create
        );
    }

    #[test]
    fn resolve_action_yes_is_idempotent_update() {
        assert_eq!(
            resolve_action(Some(&existing()), false, false, true),
            ActionChoice::Update(42)
        );
    }

    #[test]
    fn resolve_action_conflict_when_unforced() {
        assert_eq!(
            resolve_action(Some(&existing()), false, false, false),
            ActionChoice::Conflict(existing())
        );
    }
}
