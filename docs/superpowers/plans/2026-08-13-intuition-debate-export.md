# Intuition debate export Implementation Plan

> **For agentic workers:** Execute inline in this session (user: « allons y »).

**Goal:** Plan, export, and optionally publish situated debates to Intuition testnet without putting map facts on-chain.

**Architecture:** Rust canon crate (pure) → store queue → CLI. `--live` shells to a TS sidecar copied from the Talaria POC orchestrator.

**Tech Stack:** Rust, sqlx, Node 20+, `@0xintuition/sdk`, viem, Intuition testnet 13579.

## Global Constraints

- Cultural events stay in Talaria DB; Intuition gets question/proposition (+ about event pointer).
- String atoms (`question:…`) this slice — no IPFS pin.
- Testnet only. `--live` needs `INTUITION_PRIVATE_KEY`.
- Do not write the unused 005 `claims` table.
- Code files start with `// path` one-line comment.
- Stay on branch `dev`.

## Files

- Create: `crates/talaria-intuition/**`
- Create: `migrations/017_intuition_publications.sql`
- Create: `crates/talaria-store/src/intuition.rs`
- Create: `crates/talaria-api/src/intuition.rs`
- Create: `sidecar/intuition/writeOnChain.ts`, `package.json`
- Modify: workspace `Cargo.toml`, `talaria-store/src/lib.rs`, `talaria-api` cli/main/Cargo.toml, `.env.example`

### Task 1: Canon crate

Port Canonicalizer tests from POC. `normalize_slug_fragment`, `full_slug`, `build_debate_bundle`, `situated_context_triples`, place-conflict planner.

### Task 2: Queue + CLI dry-run

Migration 017, list conflict/soft-claim rows, `intuition-plan` / `intuition-export`.

### Task 3: Sidecar `--live`

POC writer, persist term_id/tx, fail clearly without key/npm.

### Verify

`cargo test -p talaria-intuition && cargo build -p talaria-api`
