# Releasing

Releases are automated with [release-plz](https://release-plz.dev). The whole flow is
trunk-based: you never tag or publish by hand, you just merge a release PR.

## The flow

1. **Push to `master`.** `release-plz` opens (or updates) a **release PR** that bumps the
   version in `Cargo.toml` and updates `CHANGELOG.md` based on the conventional commits since
   the last release.
2. **Merge the release PR.** On the resulting push to `master`, `release-plz`:
   - publishes the crate to **crates.io** (`cargo publish`),
   - creates the **`v{version}` git tag**,
   - creates the **GitHub Release**.
3. Cutting that release flips the `releases_created` output to `true`, which fans out to:
   - **`build`** — cross-compiles the release binary for each Homebrew target,
   - **`upload-assets`** — attaches the tarballs to the GitHub Release,
   - **`update-tap`** — renders the formula and pushes it to `getkono/homebrew-tap`.

If `master` has no unreleased version (i.e. the current `Cargo.toml` version is already on
crates.io), `release-plz` cuts nothing and the build/Homebrew jobs are skipped by design — that
is the expected no-op, not a failure.

## Required secrets

Set these in **Settings → Secrets and variables → Actions**:

| Secret | Required for | Notes |
| --- | --- | --- |
| `CARGO_REGISTRY_TOKEN` | crates.io publish | A crates.io API token. **Without it the release job fails** at `cargo publish`, which aborts the tag, GitHub Release, and Homebrew update too. |
| `HOMEBREW_TAP_TOKEN` | Homebrew tap update | A PAT with write access to `getkono/homebrew-tap`. If absent, `update-tap` is a clean no-op (the rest of the release still succeeds). |
| `RELEASE_PLZ_TOKEN` | optional | A maintainer PAT. Lets CI run on the release PR (so branch protection can gate it) and shows the maintainer as the GitHub Release author. Falls back to the built-in `GITHUB_TOKEN`. |

The built-in `GITHUB_TOKEN` already covers the git tag and the GitHub Release, so those two
work with no extra setup.

## First / manual release

The `release-plz.yml` workflow also has a `workflow_dispatch` trigger, so the first release (or
any re-run) can be kicked off by hand from the **Actions** tab without waiting for a release PR.

## Current state (note)

`sendit 0.1.0` was published to crates.io out of band, but **without** a matching `v0.1.0` tag,
GitHub Release, or Homebrew formula. `release-plz` therefore treats 0.1.0 as already released and
will not re-cut it. The next full release — all four artifacts together — comes from merging the
open `chore: release v0.1.1` PR, **after** `CARGO_REGISTRY_TOKEN` (and `HOMEBREW_TAP_TOKEN`, for
the tap) are added to repo secrets.
