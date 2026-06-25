use std::fmt::Write as _;
use std::io::Write as _;
use std::process::Command;

use crate::context::PrContext;
use crate::error::SenditError;

/// Marker separating the editable region from the ignored context block.
/// Mirrors git's `commit --verbose` scissors line.
pub const SCISSORS: &str = "# ------------------------ >8 ------------------------";

/// A PR title and body extracted from editor output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedPr {
    pub title: String,
    pub body: String,
}

/// Render the editor template for a context, with no seeded title or body.
pub fn render_template(ctx: &PrContext) -> String {
    render(ctx, "", "")
}

fn render(ctx: &PrContext, title_seed: &str, body_seed: &str) -> String {
    let mut s = String::new();
    s.push_str(title_seed);
    s.push('\n');
    s.push('\n');
    if !body_seed.is_empty() {
        s.push_str(body_seed);
        s.push('\n');
        s.push('\n');
    }
    s.push_str(SCISSORS);
    s.push('\n');
    s.push_str("# Do not modify or remove the line above. Everything below it is ignored.\n");
    s.push_str("# The first non-blank line above is the PR title; the rest is the body.\n");
    s.push_str("# An empty title aborts (nothing is pushed or created).\n");
    s.push_str("#\n");
    let _ = writeln!(
        s,
        "# Branch: {} -> {}   |   Commits ahead: {}",
        ctx.branch, ctx.trunk, ctx.commits_ahead
    );
    if !ctx.commit_log.is_empty() {
        s.push_str("#\n# Commits:\n");
        for c in &ctx.commit_log {
            let _ = writeln!(s, "#   {} {}", c.hash, c.subject);
        }
    }
    if !ctx.diffstat.raw.is_empty() {
        s.push_str("#\n# Diff stat:\n");
        for line in ctx.diffstat.raw.lines() {
            let _ = writeln!(s, "#   {line}");
        }
    }
    s
}

/// Parse raw editor output into a title and body.
///
/// Everything from the scissors line onward is discarded. The first non-blank
/// remaining line is the title; the rest (with surrounding blank lines trimmed)
/// is the body. Returns `EmptyTitle` when no title line is present.
pub fn parse_editor_output(raw: &str) -> Result<ParsedPr, SenditError> {
    let content: Vec<&str> = raw
        .lines()
        .take_while(|line| line.trim_end() != SCISSORS)
        .collect();

    let Some(idx) = content.iter().position(|l| !l.trim().is_empty()) else {
        return Err(SenditError::EmptyTitle);
    };

    let title = content[idx].trim().to_string();
    let body = content[idx + 1..].join("\n");
    let body = body.trim_matches('\n').trim_end().to_string();

    Ok(ParsedPr { title, body })
}

/// Split a `$EDITOR` value into a program and its arguments.
///
/// `"code --wait"` becomes `("code", ["--wait"])`. An empty value falls back to `vi`.
pub fn split_editor_command(editor: &str) -> (String, Vec<String>) {
    let mut parts = editor.split_whitespace();
    let program = parts.next().unwrap_or("vi").to_string();
    let args = parts.map(str::to_string).collect();
    (program, args)
}

/// Resolve the editor command from the environment (`EDITOR` → `VISUAL` → `vi`).
fn editor_command() -> String {
    std::env::var("EDITOR")
        .or_else(|_| std::env::var("VISUAL"))
        .unwrap_or_else(|_| "vi".to_string())
}

/// Open the editor on a template (optionally seeded with a title and/or body)
/// and parse the result.
pub fn compose_via_editor(
    ctx: &PrContext,
    title_seed: &str,
    body_seed: &str,
) -> Result<ParsedPr, SenditError> {
    let template = render(ctx, title_seed, body_seed);

    let mut file = tempfile::Builder::new()
        .prefix("SENDIT_PR_")
        .suffix(".md")
        .tempfile()?;
    file.write_all(template.as_bytes())?;
    file.flush()?;
    let path = file.path().to_path_buf();

    let editor = editor_command();
    let (program, mut args) = split_editor_command(&editor);
    args.push(path.to_string_lossy().into_owned());

    let status = Command::new(&program).args(&args).status()?;
    if !status.success() {
        return Err(SenditError::EditorFailed(editor));
    }

    let edited = std::fs::read_to_string(&path)?;
    parse_editor_output(&edited)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::{CommitEntry, DiffStat};

    fn ctx() -> PrContext {
        PrContext {
            branch: "feat/foo".to_string(),
            trunk: "master".to_string(),
            merge_base: "abc".to_string(),
            has_upstream: false,
            commits_ahead: 2,
            commit_log: vec![CommitEntry {
                hash: "a1b2c3d".to_string(),
                subject: "Add the widget loader".to_string(),
            }],
            diffstat: DiffStat {
                files: 1,
                insertions: 42,
                deletions: 7,
                raw: " src/widget.rs | 49 +++++++---".to_string(),
            },
            existing_pr: None,
        }
    }

    #[test]
    fn template_contains_context() {
        let t = render_template(&ctx());
        assert!(t.contains(SCISSORS));
        assert!(t.contains("# Branch: feat/foo -> master   |   Commits ahead: 2"));
        assert!(t.contains("#   a1b2c3d Add the widget loader"));
        assert!(t.contains("src/widget.rs | 49 +++++++---"));
        assert!(t.contains("# Diff stat:"));
    }

    #[test]
    fn parse_title_and_body() {
        let raw = "Add JWT auth\n\n## Summary\n- did the thing\n";
        let p = parse_editor_output(raw).unwrap();
        assert_eq!(p.title, "Add JWT auth");
        assert_eq!(p.body, "## Summary\n- did the thing");
    }

    #[test]
    fn parse_title_only() {
        let p = parse_editor_output("Add JWT auth\n").unwrap();
        assert_eq!(p.title, "Add JWT auth");
        assert_eq!(p.body, "");
    }

    #[test]
    fn parse_drops_scissors_block() {
        let raw = format!("My title\n\nbody line\n{SCISSORS}\n# Branch: x -> y\n#   commitline\n");
        let p = parse_editor_output(&raw).unwrap();
        assert_eq!(p.title, "My title");
        assert_eq!(p.body, "body line");
    }

    #[test]
    fn parse_template_untouched_is_empty_title() {
        let raw = render_template(&ctx());
        assert!(matches!(
            parse_editor_output(&raw),
            Err(SenditError::EmptyTitle)
        ));
    }

    #[test]
    fn parse_preserves_markdown_headers_in_body() {
        let raw = "Title\n\n# A heading\n## Another\nbody\n";
        let p = parse_editor_output(raw).unwrap();
        assert_eq!(p.body, "# A heading\n## Another\nbody");
    }

    #[test]
    fn parse_skips_leading_blank_lines() {
        let p = parse_editor_output("\n\n  Title here  \n\nbody\n").unwrap();
        assert_eq!(p.title, "Title here");
        assert_eq!(p.body, "body");
    }

    #[test]
    fn parse_preserves_backticks_and_vars() {
        let raw = "Title\n\nuse `cargo build` and $HOME and \"quotes\"\n";
        let p = parse_editor_output(raw).unwrap();
        assert_eq!(p.body, "use `cargo build` and $HOME and \"quotes\"");
    }

    #[test]
    fn split_simple_editor() {
        assert_eq!(split_editor_command("vi"), ("vi".to_string(), vec![]));
    }

    #[test]
    fn split_editor_with_args() {
        assert_eq!(
            split_editor_command("code --wait"),
            ("code".to_string(), vec!["--wait".to_string()])
        );
        assert_eq!(
            split_editor_command("  emacs -nw  "),
            ("emacs".to_string(), vec!["-nw".to_string()])
        );
    }

    #[test]
    fn split_empty_editor_defaults_vi() {
        assert_eq!(split_editor_command(""), ("vi".to_string(), vec![]));
    }
}
