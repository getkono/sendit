# Releasing

Releases are automated with [release-plz](https://release-plz.dev). The whole flow is
trunk-based: you never tag or publish by hand, you just merge a release PR.

## The flow

1. **Push to `master`.** `release-plz` opens (or updates) a **release PR** that bumps the
   version in `Cargo.toml` and updates `CHANGELOG.md` based on the conventional commits since
   the last release.
2. **Merge the release PR.** On the resulting push to `master`, `release-plz`:
   - publishes the crate to **crates.io** (`cargo publish`, authenticated via Trusted
     Publishing — see below),
   - creates the **`v{version}` git tag**,
   - creates the **GitHub Release**.
3. Cutting that release flips the `releases_created` output to `true`, which fans out to:
   - **`build`** — cross-compiles the release binary for each Homebrew target,
   - **`upload-assets`** — attaches the tarballs to the GitHub Release,
   - **`update-tap`** — renders the formula and pushes it to `getkono/homebrew-tap`.

If `master` has no unreleased version (i.e. the current `Cargo.toml` version is already on
crates.io), `release-plz` cuts nothing and the build/Homebrew jobs are skipped by design — that
is the expected no-op, not a failure.

## crates.io: Trusted Publishing (no token secret)

Publishing uses crates.io [Trusted Publishing](https://crates.io/docs/trusted-publishing)
(OIDC), so there is **no `CARGO_REGISTRY_TOKEN` secret to store or rotate**. The release job
requests `id-token: write`, the `rust-lang/crates-io-auth-action` step exchanges the job's OIDC
identity for a short-lived token, and release-plz reads it from `CARGO_REGISTRY_TOKEN`.

This only works because a Trusted Publisher is configured for the `sendit` crate on crates.io.
If it ever needs re-creating (crates.io → crate **Settings → Trusted Publishing → Add**), use:

| Field | Value |
| --- | --- |
| Repository owner | `getkono` |
| Repository name | `sendit` |
| Workflow filename | `release-plz.yml` |
| Environment | *(leave blank)* |

## Secrets

| Secret | Required? | Purpose |
| --- | --- | --- |
| `HOMEBREW_TAP_TOKEN` | **Yes** | A PAT with write access to `getkono/homebrew-tap`, so `update-tap` can push the formula. If absent, `update-tap` is a clean no-op and the rest of the release still succeeds. |

That's the only secret. Notes on the two you might expect to see:

- **`CARGO_REGISTRY_TOKEN` — not used.** Replaced by Trusted Publishing (above).
- **`RELEASE_PLZ_TOKEN` — not used.** A maintainer PAT would only matter if `master` were
  branch-protected with a required status check on the release PR: PRs opened by the built-in
  `GITHUB_TOKEN` don't trigger CI, so the required check could never go green. `master` is not
  protected, so nothing gates on it. The release itself (tag + GitHub Release) runs on the
  built-in `GITHUB_TOKEN`. The only thing a PAT would buy is showing you, rather than
  `github-actions[bot]`, as the Release author. If you ever add branch protection with required
  checks, add a PAT then and wire it into both release-plz jobs as `GITHUB_TOKEN`.

## First / manual release

The `release-plz.yml` workflow also has a `workflow_dispatch` trigger, so the first release (or
any re-run) can be kicked off by hand from the **Actions** tab without waiting for a release PR.

## Current state (note)

`sendit 0.1.0` was published to crates.io out of band, but **without** a matching `v0.1.0` tag,
GitHub Release, or Homebrew formula. `release-plz` therefore treats 0.1.0 as already released and
will not re-cut it. The next full release — all four artifacts together — comes from merging the
open `chore: release v0.1.1` PR.
