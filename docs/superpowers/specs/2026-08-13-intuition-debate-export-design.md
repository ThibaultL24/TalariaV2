# Intuition debate export (TalariaV2)

Date: 2026-08-13
Status: approved (chat) — debates situés, JSON then `--live` testnet

## Goal

Publish **avis / théories / débats** to Intuition as canonical triples, with a pointer to a Talaria event. Cultural map facts stay in Postgres.

## Non-goals

- Life-event triples (`born_on`, battles as facts)
- IPFS pin (POC used string atoms; pin later)
- Mainnet, wallet UI, Agora GitHub gate

## Triple shape (POC Canonicalizer)

- `question —has-proposition→ proposition` (vote target)
- `proposition —about→ event:canonical-event-{uuid}` (pointer, not the fact)
- optional `event —at→ place`
- Atoms are string slugs: `kind:hyphenated-fragment`
- Schema version: `talaria.intuition_canon.v1`

## Sources (never table `claims` 005 — unused)

1. `quality_claims.status='conflict'` grouped by `occurrence_stem` → one question, one proposition per distinct place
2. `soft_claims` where `claim_kind` ∈ `theory`, `controversy`, `debate_stance`

## Queue

Table `intuition_publications`: bundle fingerprint, payload JSON, status, on-chain `term_id` / tx. Idempotent.

## CLI

- `talaria intuition-plan --subject …`
- `talaria intuition-export --subject …` (JSON stdout + persist `exported`)
- `talaria intuition-publish --subject … --live` (testnet 13579, server key, sidecar TS)

## Network

Default **Intuition Testnet** chain 13579. `--live` requires `INTUITION_PRIVATE_KEY`. Dry-run never talks to RPC.

## Split

- `talaria-intuition`: slugs, bundles, plan from in-memory rows (no DB, no RPC)
- `talaria-store` + API: load rows, persist publications, spawn sidecar
- `sidecar/intuition`: `@0xintuition/sdk` write (POC `writeIntuitionOnChain`)
