use crate::context::PrContext;
use crate::pr::{PrAction, PrOutcome, PrSpec};

/// Format the final "Sent!" summary block.
pub fn format_summary(outcome: &PrOutcome, ctx: &PrContext, spec: &PrSpec) -> String {
    let action = match outcome.action {
        PrAction::Create => "Created",
        PrAction::Update { .. } => "Updated",
    };
    let status = if outcome.draft { "Draft" } else { "Ready" };
    let stat = &ctx.diffstat;
    format!(
        "Sent! {action} PR\n  Title:   {title}\n  URL:     {url}\n  Status:  {status}\n  Branch:  {branch} -> {trunk}\n  Commits: {commits} | Files: {files} | Changes: +{ins} -{del}",
        title = spec.title,
        url = outcome.url,
        branch = ctx.branch,
        trunk = ctx.trunk,
        commits = ctx.commits_ahead,
        files = stat.files,
        ins = stat.insertions,
        del = stat.deletions,
    )
}

/// Print the final summary to stdout.
pub fn print_summary(outcome: &PrOutcome, ctx: &PrContext, spec: &PrSpec) {
    println!("{}", format_summary(outcome, ctx, spec));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::DiffStat;

    fn ctx() -> PrContext {
        PrContext {
            branch: "feat/foo".to_string(),
            trunk: "master".to_string(),
            merge_base: "abc".to_string(),
            has_upstream: true,
            commits_ahead: 3,
            commit_log: vec![],
            diffstat: DiffStat {
                files: 2,
                insertions: 10,
                deletions: 4,
                raw: String::new(),
            },
            existing_pr: None,
        }
    }

    #[test]
    fn summary_created_ready() {
        let spec = PrSpec {
            title: "Add auth".to_string(),
            body: "body".to_string(),
            draft: false,
        };
        let outcome = PrOutcome {
            url: "https://github.com/o/r/pull/9".to_string(),
            number: Some(9),
            draft: false,
            action: PrAction::Create,
        };
        let s = format_summary(&outcome, &ctx(), &spec);
        assert!(s.contains("Created PR"));
        assert!(s.contains("Title:   Add auth"));
        assert!(s.contains("URL:     https://github.com/o/r/pull/9"));
        assert!(s.contains("Status:  Ready"));
        assert!(s.contains("Branch:  feat/foo -> master"));
        assert!(s.contains("Commits: 3 | Files: 2 | Changes: +10 -4"));
    }

    #[test]
    fn summary_updated_draft() {
        let spec = PrSpec {
            title: "Add auth".to_string(),
            body: "body".to_string(),
            draft: true,
        };
        let outcome = PrOutcome {
            url: "https://github.com/o/r/pull/9".to_string(),
            number: Some(9),
            draft: true,
            action: PrAction::Update { number: 9 },
        };
        let s = format_summary(&outcome, &ctx(), &spec);
        assert!(s.contains("Updated PR"));
        assert!(s.contains("Status:  Draft"));
    }
}
