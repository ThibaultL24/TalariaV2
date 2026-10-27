# Talaria — development workflow for AI agents

This protocol applies equally to ChatGPT Work, Codex and Cursor. `main` is the integration branch. The canonical historical-data constraints remain in `AGENTS.md`.

## Scope and acceptance

1. Inspect the affected code, interfaces, migrations and nearby tests **before** proposing changes.
2. Define the observable outcome and invariants: no invented historical evidence, dates, places or coordinates; no new Explorer data lane; no breaking API contract without explicit design.
3. For larger work, prepare a short plan, then implement a small independently testable lot. Keep architecture discussions separate from code changes.

## Git and pull requests

1. Branch from the intended, current base. Work in a dedicated task branch. **Do not commit directly to main.**
2. Keep each PR narrowly scoped; describe the baseline, files changed, schema/API impacts, risks, commands actually run and any validation not performed.
3. Stacked PRs must name their true base branch and explain merge order. Verify CI on each stack level; GitHub's default main-base checks alone are not enough.
4. A review agent should inspect **the diff**, not regenerate an unrelated solution. Address substantive review findings.
5. Only merge after checks and human review; deployment and destructive database actions require explicit authorization.

## Validation matrix

| Change | Local checks (as appropriate) |
| --- | --- |
| React / TypeScript | `cd web && npm test && npm run build`; browser smoke test on desktop/mobile |
| Rust source / quality | `cargo check -p <crate>`, `cargo test -p talaria-quality`, `cargo test -p talaria-sources` |
| Rust API / store | Targeted `cargo test -p talaria-api <module>` and `cargo test -p talaria-store <module>`; DB-backed tests need PostGIS |
| Migrations | Additive migrations, PostGIS integration tests, backup/rollback strategy before production |
| UX / map / timeline | In addition to automated tests: real browser, responsive, keyboard access, reduced motion and representative large datasets |
| Tourist live data | Provenance, freshness, missing information, rate limits and access controls |
| Historical narration | Every factual assertion traceable to evidence; uncertainty and interpretation visibly distinguished |

GitHub Actions workflow `.github/workflows/v3.yml` is a baseline, not proof that every test passes. Check actual workflow runs before merging. A test skipped due to missing credentials or environment must be recorded as **not run**.

## Branch protection (GitHub repository setting)

On `main`, enable protection/ruleset: require pull requests, require relevant successful CI checks (`rust` and `web` after they have run), and block force pushes/deletion. Use an explicit bypass policy only if strictly necessary. These settings live in GitHub and cannot be enforced by this document alone.

## Generated and large data

`node_modules/`, `.cache/`, build outputs, DB dumps and ephemeral test caches must not enter new commits. Previously tracked files require a deliberate one-time removal from the Git index. The ~90MB Mérimée CSV in `.cache/` must first be checked for runtime or operator dependencies and assigned an external data source before being untracked; do not silently delete corpus inputs.

## PR inventory

Periodically reconcile open PRs (mergeable, conflicting, stacked, superseded) against `main`; do not mass-merge or close unpublished work without checking its contents. Preserve historical data and snapshot integrity.
