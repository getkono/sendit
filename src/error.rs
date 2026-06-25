use thiserror::Error;

/// Errors produced by the `sendit` library.
///
/// Variants fall into three buckets that the binary maps to distinct exit codes:
/// preflight refusals (the repo/branch isn't in a state to PR), the `Aborted`
/// and `EmptyTitle` user-cancellation cases, and operational failures from the
/// underlying `git`/`gh` processes.
#[derive(Debug, Error)]
pub enum SenditError {
    #[error("not inside a git repository")]
    NotARepo,

    #[error("GitHub CLI not authenticated; run `gh auth login`")]
    NotAuthenticated,

    #[error("`gh` not found on PATH; install the GitHub CLI (https://cli.github.com)")]
    GhNotInstalled,

    #[error("`git` not found on PATH")]
    GitNotInstalled,

    #[error("refusing to open a PR from trunk branch `{0}`; check out a feature branch first")]
    OnTrunk(String),

    #[error("detached HEAD; check out a branch first")]
    DetachedHead,

    #[error("could not determine the trunk/default branch; pass --base <ref>")]
    NoTrunk,

    #[error("no commits ahead of `{trunk}`; nothing to send")]
    NothingToSend { trunk: String },

    #[error("git command failed: {0}")]
    GitFailed(String),

    #[error("gh command failed: {0}")]
    GhFailed(String),

    #[error("failed to parse gh JSON output: {0}")]
    GhParse(String),

    #[error("editor `{0}` exited with an error")]
    EditorFailed(String),

    #[error("empty PR title; aborting (nothing was pushed or created)")]
    EmptyTitle,

    #[error("a PR title is required: pass --title, or run interactively to use the editor")]
    TitleRequired,

    #[error("aborted by user")]
    Aborted,

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}
