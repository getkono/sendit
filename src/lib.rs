//! `sendit` — push the current branch and open a GitHub PR with a title and body
//! you write yourself (no LLM). The binary is a thin wrapper over [`run`]; the
//! library exposes the preflight/context/PR primitives for embedding in other tools.

pub mod cli;
pub mod context;
pub mod editor;
pub mod error;
pub mod gh;
pub mod git;
pub mod pr;
pub mod summary;

pub use cli::Cli;
pub use context::{preflight, resolve_trunk, PrContext};
pub use editor::{compose_via_editor, parse_editor_output, render_template, ParsedPr};
pub use error::SenditError;
pub use gh::ExistingPr;
pub use git::{parse_commit_log, parse_shortstat, CommitEntry, DiffStat};
pub use pr::{
    build_create_args, build_edit_args, create_or_update_pr, parse_pr_number, parse_pr_url,
    resolve_action, ActionChoice, PrAction, PrOutcome, PrSpec,
};
pub use summary::format_summary;

use std::io::{self, IsTerminal, Read, Write};

/// Entry point for the binary. Runs the full flow and returns a process exit code:
/// `0` success, `1` empty title, `2` preflight refusal, `3` operational failure,
/// `130` user abort.
pub fn run(cli: Cli) -> i32 {
    match run_inner(&cli) {
        Ok(()) => 0,
        Err(err) => {
            eprintln!("sendit: {err}");
            match err {
                SenditError::Aborted => 130,
                SenditError::EmptyTitle => 1,
                SenditError::NotARepo
                | SenditError::DetachedHead
                | SenditError::OnTrunk(_)
                | SenditError::NoTrunk
                | SenditError::NothingToSend { .. }
                | SenditError::NotAuthenticated
                | SenditError::GhNotInstalled
                | SenditError::GitNotInstalled
                | SenditError::TitleRequired => 2,
                _ => 3,
            }
        }
    }
}

fn run_inner(cli: &Cli) -> Result<(), SenditError> {
    let ctx = preflight(cli.base.as_deref())?;
    let action = decide_action(cli, &ctx)?;
    let spec = resolve_spec(cli, &ctx)?;

    if !cli.yes {
        confirm(&ctx, action, &spec)?;
    }

    git::push(&ctx.branch, !ctx.has_upstream)?;
    let outcome = create_or_update_pr(&ctx, &spec, action)?;
    summary::print_summary(&outcome, &ctx, &spec);
    Ok(())
}

/// Choose create vs update when a PR already exists for this branch.
///
/// The flag-driven decision is delegated to the pure [`resolve_action`]; only
/// the unforced `Conflict` case prompts interactively here.
fn decide_action(cli: &Cli, ctx: &PrContext) -> Result<PrAction, SenditError> {
    match resolve_action(ctx.existing_pr.as_ref(), cli.update, cli.new, cli.yes) {
        ActionChoice::Create => Ok(PrAction::Create),
        ActionChoice::Update(number) => Ok(PrAction::Update { number }),
        ActionChoice::Conflict(existing) => {
            eprint!(
                "PR #{} already exists ({}) {}\n[u]pdate / [c]reate new / [s]top (default s): ",
                existing.number, existing.state, existing.url
            );
            io::stderr().flush()?;
            match read_line()?.trim().to_lowercase().as_str() {
                "u" | "update" => Ok(PrAction::Update {
                    number: existing.number,
                }),
                "c" | "create" | "new" => Ok(PrAction::Create),
                _ => Err(SenditError::Aborted),
            }
        }
    }
}

/// Resolve the PR title/body, falling back to the editor when needed.
fn resolve_spec(cli: &Cli, ctx: &PrContext) -> Result<PrSpec, SenditError> {
    let flag_body = read_flag_body(cli)?;
    let interactive = !cli.yes && io::stdin().is_terminal();

    // Both supplied via flags: no editor.
    if let (Some(title), Some(body)) = (&cli.title, &flag_body) {
        return Ok(PrSpec {
            title: title.clone(),
            body: body.clone(),
            draft: cli.draft,
        });
    }

    if !interactive {
        // Non-interactive: a title must come from a flag; body defaults to empty.
        let Some(title) = &cli.title else {
            return Err(SenditError::TitleRequired);
        };
        return Ok(PrSpec {
            title: title.clone(),
            body: flag_body.unwrap_or_default(),
            draft: cli.draft,
        });
    }

    // Interactive: open the editor, seeding whatever flags were provided.
    let parsed = compose_via_editor(
        ctx,
        cli.title.as_deref().unwrap_or(""),
        flag_body.as_deref().unwrap_or(""),
    )?;
    Ok(PrSpec {
        title: parsed.title,
        body: parsed.body,
        draft: cli.draft,
    })
}

/// Read the PR body from `--body`, `--body-file <path>`, or `--body-file -` (stdin).
fn read_flag_body(cli: &Cli) -> Result<Option<String>, SenditError> {
    if let Some(body) = &cli.body {
        return Ok(Some(body.clone()));
    }
    let Some(path) = &cli.body_file else {
        return Ok(None);
    };
    if path == "-" {
        let mut buf = String::new();
        io::stdin().read_to_string(&mut buf)?;
        return Ok(Some(buf));
    }
    Ok(Some(std::fs::read_to_string(path)?))
}

/// Final y/N gate before pushing and creating/updating the PR.
fn confirm(ctx: &PrContext, action: PrAction, spec: &PrSpec) -> Result<(), SenditError> {
    let verb = match action {
        PrAction::Create => "create".to_string(),
        PrAction::Update { number } => format!("update #{number}"),
    };
    // Draft is only meaningful on create (gh pr edit can't toggle it).
    let draft = if spec.draft && matches!(action, PrAction::Create) {
        " draft"
    } else {
        ""
    };
    eprint!(
        "Push {} and {verb}{draft} PR -> {}. Continue? [y/N]: ",
        ctx.branch, ctx.trunk
    );
    io::stderr().flush()?;
    match read_line()?.trim().to_lowercase().as_str() {
        "y" | "yes" => Ok(()),
        _ => Err(SenditError::Aborted),
    }
}

/// Read one line from stdin; EOF yields an empty string.
fn read_line() -> Result<String, SenditError> {
    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    Ok(line)
}
