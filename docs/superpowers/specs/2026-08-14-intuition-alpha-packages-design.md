# Intuition Alpha packages export (TalariaV2)

Date: 2026-08-14
Status: approved (chat) — v1 for testers; sidecar Alpha packages

## Goal

Publish **avis / théories / débats** to Intuition testnet using MetaSudo Alpha packages (`@0xintuition/classifications`, `predicates`, `ids`, `primitives`, `protocol` 3.x, `deployments`). The sidecar owns the graph. Rust selects debates and owns the queue. Cultural map facts stay in Postgres.

## Decisions (frozen)

- Sidecar owns atoms, triples, and deterministic IDs (approach A).
- Classify the debate, not the historical fact (option 1). No life-event triples, no `locatedIn`.
- Structured atoms: pin JSON-LD on `--live` only (approach A). `plan` / `export` are off-chain.
- Schema `talaria.intuition_canon.v2`. v1 slug rows are never republished and never migrated.

## Non-goals

- Life-event triples (`born_on`, `fought_at`, battles as attested facts)
- `locatedIn` / `organizer` / `performer` on the Event pointer
- Person / Place identity graph (option 2)
- `intuition-core` backend, mainnet, wallet UI, Agora GitHub gate
- Rewriting or requalifying `pipeline='legacy'` events
- Writing the unused `claims` table (migration 005)
- Live chain tests in CI

## Ownership

| Layer | Owns |
| --- | --- |
| Postgres / quality pipeline | Cultural facts (`canonical_events`), opinion rows (`quality_claims` conflict, `soft_claims`) |
| `talaria-api` + `talaria-store` | Collect `DebateFact`, queue `intuition_publications`, spawn sidecar, persist status / term ids |
| `talaria-intuition` | Pure `DebateFact` shaping + fact fingerprint (no slugs, no RPC) |
| `sidecar/intuition` | Model (classifications / primitives / ids / predicates), pin, settle (protocol 3.x + deployments) |

## Sources

Never table `claims` (005).

1. `quality_claims.status='conflict'` grouped by `occurrence_stem` → one question, one proposition per distinct place
2. `soft_claims` where `claim_kind` ∈ `theory`, `controversy`, `debate_stance`

## DebateFact

Rust → sidecar. No house slugs.

```json
{
  "version": "talaria.intuition_canon.v2",
  "debate_id": "talaria:debate:…",
  "kind": "place_conflict",
  "question": { "text": "Where was Napoleon during battle (1805)?" },
  "proposition": { "text": "Austerlitz" },
  "about_event": {
    "canonical_event_id": "uuid",
    "title": "Battle of Austerlitz",
    "event_type": "battle",
    "time_surface": "1805-12-02"
  }
}
```

- `kind`: `place_conflict` | `theory` | `controversy` | `debate_stance`
- `about_event` optional. No coordinates. No chosen place on the event pointer (the proposition carries the place claim).
- `time_surface` is copied as typed (`1805`, `1805-12`, `1805-12-02`). Never coerce a year to `YYYY-01-01`.

### Fact fingerprint

SHA-256 of canonical JSON (sorted keys) over identity fields only:

`version`, `kind`, `question.text`, `proposition.text`, `about_event.canonical_event_id` (or null)

Exclude display `title`. Unique key of the queue: `bundle_fingerprint` = this hash. A text or event-id change is a new row. Title-only changes do not fork the row.

## Graph (sidecar)

Packages: `@0xintuition/classifications@alpha`, `@0xintuition/predicates@alpha`, `@0xintuition/ids@alpha`, `@0xintuition/primitives@alpha`.

### Atoms

| Atom | Classification | Fields |
| --- | --- | --- |
| question | `defined-term` | `name` = question text |
| proposition | `defined-term` | `name` = proposition text |
| category | `defined-term` | `name` = category term (table below) |
| event | `event` | only if `about_event` present. `name` = frozen `canonical-event:{uuid}` (stable identity). `sameAs` = `talaria://canonical-event/{uuid}`. `startDate` only when `time_surface` is a full calendar date (`YYYY-MM-DD`). **No** `location`. |

Human title is stored on the queue payload for logs; it is not Event atom identity (title is derived and can change).

Without `about_event`: vote + category only. No Event atom.

### Category mapping

Primary `hasCategory` term:

| Input | DefinedTerm name |
| --- | --- |
| `about_event.event_type` present | that type, normalized (`battle`, `diplomatic`, `exile`, `residence`, `marriage`, `death`, …) |
| else `kind` | `place_conflict` / `theory` / `controversy` / `debate_stance` |
| unknown / empty type | `uncategorized` |

If both `event_type` and a non-`place_conflict` `kind` exist, also `question —hasTag→` DefinedTerm(`kind`).

Unknown Talaria types pass through as their own DefinedTerm. Do not invent a schema.org `Battle` type; Alpha has `event`, not battle.

### Triples (opinions only)

1. **Vote target:** `question —hasProposition→ proposition`. Prefer registry predicate; if absent, `createPredicateAtomData('hasProposition', …)` via `@0xintuition/ids`.
2. **Pointer:** `proposition —about→ event` when `about_event` exists. Registry `about`, else ids helper.
3. **Classification:** `question —hasCategory→ category` (and optional `hasTag` above).

Forbidden: `locatedIn`, `organizer`, `performer`, `born_on`, `fought_at`, `died_in`, any triple that asserts a map fact.

IDs from `@0xintuition/ids` **before** pin and chain. Vote triple ID is the on-chain identity of the publication.

## Queue and CLI

Table `intuition_publications` (migration 017 + additive 019).

CLI unchanged in name:

- `talaria intuition-plan --subject …` — collect + sidecar **model only** → persist `pending`
- `talaria intuition-export --subject …` — same as plan, JSON stdout, status `pending`
- `talaria intuition-publish --subject … --live` — pin then settle

Dry-run never talks to RPC or pin. `--live` requires `INTUITION_PRIVATE_KEY` (`0x` + 64 hex). Testnet chain **13579** only. Observed chain id ≠ 13579 → abort before writes.

v1 rows (`payload_json.version` ≠ `talaria.intuition_canon.v2`, or slug bundles): skip. Do not migrate slugs into Alpha IDs.

### Status values (v2)

`pending` | `pin_failed` | `failed` | `published`

Legacy CHECK values `planned` / `exported` remain readable for v1 rows. v2 writer never sets them.

## State transitions

Live publish starts from a retryable status and ends in one terminal or retryable result. `published` is absorbing.

```
plan/export (model ok)
        │
        ▼
    pending ──────────────────────────────────────────────► published
        │                                                      ▲
        │ pin error                                            │
        ├──────────────────► pin_failed ─── retry --live ──────┤
        │                                                      │
        │ settle error (pin succeeded or atoms already exist)  │
        └──────────────────► failed ────── retry --live ───────┘
```

| From | Event | To | Chain writes this attempt | Queue write |
| --- | --- | --- | --- | --- |
| — | model ok (`plan`/`export`) | `pending` | none | upsert payload + fingerprint; **do not** demote `published` |
| — | model error | no row / unchanged | none | no status write (CLI error) |
| `pending` | pin error | `pin_failed` | none (pin is before any `createAtoms`) | `last_error`; term ids unchanged |
| `pending` | settle error | `failed` | possible partial atoms/triples | `last_error`; may store partial term ids in payload, **not** `published` |
| `pending` | pin+settle ok, vote triple id known | `published` | yes | `chain_id`, `question_term_id`, `triple_term_id`, `tx_hash`, `last_error=NULL` |
| `pin_failed` | retry `--live` | `pin_failed` \| `failed` \| `published` | same rules as from `pending` | CAS (below) |
| `failed` | retry `--live` | `pin_failed` \| `failed` \| `published` | sidecar `isTermCreated` / ensure | CAS |
| `published` | any | `published` | none | no-op skip |
| `planned`/`exported` (v1) | v2 publish | unchanged | none | skip |

No `publishing` / in-flight status. Interruption is recovered by on-chain existence checks + CAS (next section).

`pin_failed` means: this attempt did not call MultiVault create. `failed` means: settle was attempted (partial on-chain state possible).

## Idempotence and atomic queue writes

### Identity

- Queue uniqueness: `UNIQUE (bundle_fingerprint)` on the DebateFact identity hash.
- On-chain uniqueness: vote triple ID from `@0xintuition/ids` (content-addressed). Same DebateFact → same triple ID forever.
- After first successful model, atom payloads in `payload_json` are **frozen**. Later `plan`/`export` refresh is allowed only for non-`published` rows **without** changing identity fields; title-only display fields may update in a sidecar-only log object, not in Event `name`.

### Compare-and-swap

Every status mutation is a single `UPDATE … WHERE id = $1 AND status <> 'published'` (or `INSERT … ON CONFLICT (bundle_fingerprint)` with the existing published-preserving `CASE`).

- If `published`, keep `status`, `triple_term_id`, `tx_hash`, `chain_id`. Never overwrite with `pending` / `failed` / `pin_failed`.
- Mark `published` with the same CAS predicate (`status <> 'published'`). `published` requires a non-null vote `triple_term_id`. Receipt without triple id → `failed`, not `published`.
- Pin CIDs live in `payload_json` after a successful pin so a retry does not need a new pin if the CID is already present; pin is content-addressed either way.

### Retry and interruption

| Crash / retry point | Recovery |
| --- | --- |
| Before pin | Row still `pending`. Retry pins then settles. |
| After pin, before first chain tx | Row `pending` or `pin_failed` if the process recorded the pin error. Retry: reuse CID if stored; then settle. |
| After some atoms created, process killed | Row still `pending`/`failed`. Retry: `multiVaultIsTermCreated` / ensure atom+triple; do not double-spend identity. Then CAS to `published` if vote triple exists. |
| After vote triple tx confirmed, before DB `published` | Row still `pending`/`failed`. Retry: sidecar sees term created, returns same `termId` with `created: false`. CAS writes `published` + term ids + last tx if any. |
| Concurrent `--live` on same fingerprint | Unique fingerprint + CAS: one writer wins `published`; the other skip or no-op. |
| Pin credentials missing on `--live` | `pin_failed`, zero chain writes. |

Sidecar create path is always **ensure** (exists → return id; else create). Never treat “already created” as `failed`.

Partial success is not `published`. Operators retry `--live` until `published` or until they stop. There is no automatic rollback of on-chain atoms.

## Pin and settle

On `--live` only:

1. Pin structured atom JSON-LD via the official Intuition pin HTTP API. `--live` requires pin credentials in env (`INTUITION_PIN` or the Alpha-documented equivalent). Missing credentials or pin HTTP error → `pin_failed`, zero chain writes.
2. Settle with `@0xintuition/protocol@3` + `@0xintuition/deployments` on testnet 13579. RPC failover kept (`INTUITION_RPC_URL` + fallbacks). Failure → `failed`.
3. Do not use `@0xintuition/sdk` 2.0.2 `createAtomFromString` for v2.

Dry-run (`plan`/`export`): model + ids only.

## Errors

- Missing / malformed `INTUITION_PRIVATE_KEY` → abort before sidecar spawn.
- Sidecar spawn / npm missing → CLI error, row unchanged unless already inserted `pending`.
- Pin fail → `pin_failed`.
- Settle fail → `failed`.
- `debate_count: 0` → success with empty results (nothing to queue).

## Tests

- Sidecar (vitest): `event_type` → DefinedTerm; Event `sameAs` uuid; Event `name` = `canonical-event:{uuid}`; IDs stable; **zero** `locatedIn` / life-event triples; `1805` not rewritten to `1805-01-01`; `startDate` omitted unless `YYYY-MM-DD`.
- Rust: DebateFact JSON + fingerprint stability; collect conflict + soft_claims unchanged; v1 fingerprints ignored by v2 publish.
- Store: CAS does not demote `published`; retry from `pin_failed`/`failed` can reach `published`; unique fingerprint.
- No live RPC/pin in CI.

## Supersedes

`docs/superpowers/specs/2026-08-13-intuition-debate-export-design.md` remains the v1 (slug + sdk 2.0.2) record. v2 publish must not reuse v1 `on_chain_data` slugs.
