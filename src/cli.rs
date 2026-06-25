use clap::Parser;

/// Push the current branch and open (or update) a GitHub PR.
///
/// With no flags, `sendit` opens `$EDITOR` on a pre-filled template so you can
/// type the PR title and body yourself — no LLM involved. Provide `--title` plus
/// a body source (`--body`/`--body-file`) to skip the editor for scripting.
#[derive(Debug, Parser)]
#[command(name = "sendit", version, about)]
pub struct Cli {
    /// PR title. When combined with a body source, skips the editor.
    #[arg(long, value_name = "TEXT")]
    pub title: Option<String>,

    /// PR body text.
    #[arg(long, value_name = "TEXT", conflicts_with = "body_file")]
    pub body: Option<String>,

    /// Read the PR body from a file (use `-` for stdin).
    #[arg(long, value_name = "PATH", conflicts_with = "body")]
    pub body_file: Option<String>,

    /// Create the PR as a draft (honored on create only).
    #[arg(long)]
    pub draft: bool,

    /// Skip confirmation prompts (non-interactive).
    #[arg(short = 'y', long)]
    pub yes: bool,

    /// Override the base/trunk branch to target.
    #[arg(long, value_name = "REF")]
    pub base: Option<String>,

    /// When a PR already exists for this branch, update it.
    #[arg(long, conflicts_with = "new")]
    pub update: bool,

    /// When a PR already exists for this branch, create a new one anyway.
    #[arg(long, conflicts_with = "update")]
    pub new: bool,
}

impl Cli {
    /// Whether the user supplied a body via flag (`--body` or `--body-file`).
    pub fn has_body_source(&self) -> bool {
        self.body.is_some() || self.body_file.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_args_defaults() {
        let cli = Cli::try_parse_from(["sendit"]).unwrap();
        assert!(cli.title.is_none());
        assert!(cli.body.is_none());
        assert!(cli.body_file.is_none());
        assert!(!cli.draft);
        assert!(!cli.yes);
        assert!(cli.base.is_none());
        assert!(!cli.update);
        assert!(!cli.new);
        assert!(!cli.has_body_source());
    }

    #[test]
    fn title_and_body() {
        let cli = Cli::try_parse_from(["sendit", "--title", "T", "--body", "B"]).unwrap();
        assert_eq!(cli.title.as_deref(), Some("T"));
        assert_eq!(cli.body.as_deref(), Some("B"));
        assert!(cli.has_body_source());
    }

    #[test]
    fn body_and_body_file_conflict() {
        let result = Cli::try_parse_from(["sendit", "--body", "B", "--body-file", "f.md"]);
        assert!(result.is_err());
    }

    #[test]
    fn update_and_new_conflict() {
        let result = Cli::try_parse_from(["sendit", "--update", "--new"]);
        assert!(result.is_err());
    }

    #[test]
    fn yes_and_draft() {
        let cli = Cli::try_parse_from(["sendit", "-y", "--draft"]).unwrap();
        assert!(cli.yes);
        assert!(cli.draft);
    }

    #[test]
    fn base_override() {
        let cli = Cli::try_parse_from(["sendit", "--base", "develop"]).unwrap();
        assert_eq!(cli.base.as_deref(), Some("develop"));
    }

    #[test]
    fn body_file_source() {
        let cli = Cli::try_parse_from(["sendit", "--body-file", "pr.md"]).unwrap();
        assert!(cli.body.is_none());
        assert_eq!(cli.body_file.as_deref(), Some("pr.md"));
        assert!(cli.has_body_source());
    }
}
