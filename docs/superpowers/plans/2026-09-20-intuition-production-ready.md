# Intuition × Talaria — production-ready plan

Date: 2026-09-20  
Status: **complete (phase 1–2 + cleanup G + Agora list perf)**  
Scope: Intuition lane; light perf/clean follow-ups on claims list & pipeline filters.

## Done

- Sidecar: schemas v2/v3, logical graph, predicates, resolvers, publisher, stance-deposit
- Migration `030_intuition_term_bindings` + `soft_claim_id`
- Rust `intuition-publish --live` → sidecar settle + bindings
- Stance `Ready` when published + `INTUITION_ALLOW_LIVE`; deposit execute opt-in `INTUITION_STANCE_EXECUTE=1`
- Rename person event pointers; `lot_e` / `dump_events` / `multi_source` → `pipeline='person'`
- Cap soft-claim export (80); Agora claims fetch debates-only ≤100
- Claims list: batch evidence (no N+1 corpus lookup)

## Residual

- Rust `canon`/`plan` fixtures still compiled (not on live path)
- No live chain in CI
- Deposit execute remains operator-flagged

See prior sections for architecture and payload examples (still valid).
