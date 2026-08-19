# Universal person ingest

Date: 2026-08-19  
Status: approved — 2026-08-19  
Decisions: life-trace correctness over density; all eras including living people; person-first, classes as search priors.

## Goal

Ingest a **correct life trace** for any historical or living public person. Event types, years, and places must match **that** person. Density is a by-product of sources, never a target. Napoleon is a regression fixture, not the product model.

Success is not ≥500 map points. A scientist with 80 sourced lab/publication facts is a success. 800 battles on a writer who was not a soldier is a failure.

## Non-goals

- Inventing coordinates or events to hit a density floor
- A third event pipeline
- Changing gates, `occurrence_key`, append-only quality, or `map_eligible` (still requires resolved lat/lon)
- Requalifying `pipeline='legacy'` into quality
- Intuition / Agora claim export
- Dump JSONL lots A–D (reuse as-is)
- Making class membership a hard veto on well-sourced facts about this person

## Architecture

At ingest start, resolve the subject once from Wikidata (QID):

1. Identity: label, QID, P569, P570 (absent if living), every P106.
2. Person military/conflict signals: P607, P241, P410, P710, P1344.
3. Facets: **all** matching `PersonClass` values from those properties plus the Wikipedia lead. Never pick a single winner.
4. Time window: see Time.
5. Discovery priors: union of profile boosts for those facets. Military Wikipedia BFS and WDQS conflict harvest run only if this person has a military/conflict signal.
6. Extractors: union of stacks for those facets. Military campaign extractor only if that signal is present.
7. Keep/drop after extract: person-participation rules (below), not “scientists cannot have battles”.

Gates, fingerprints, `occurrence_key`, assemble, and `resolve-places` stay the quality spine.

```text
QID → facets[] + lifespan + military_signal
    → rank/fetch pages (class boosts, person-gated military crawl)
    → extractors (union; military iff signal)
    → gates → occurrence_key → canonical_events
    → resolve-places (Wikidata P625 / aliases; no invented geom)
```

## Person-first vs classes

Classes **frame search**. They do not **limit** a real second career.

- A writer who was a soldier (P106 soldier, or P607/P710/P1344, or a clause on **their** biography “X enlisted / fought / served”) keeps military facts.
- A scientist with no such signal must not crawl “Battle of …” links from See also. Linked Waterloo on Hugo’s article is not Hugo at Waterloo.
- Keep `battle` / `siege` only if **this** person participates: WDQS P710/P1344, or a clause on the subject’s own page with this person as agent and a combat/service verb.
- Drop battles whose only link is a Wikipedia title pattern (`Battle of`, `Bataille de`) discovered from a non-participant article.

`infer_person_class` becomes `infer_person_classes` → `Vec<PersonClass>`. `ResolvedSubject::person_class()` is replaced by `person_classes()`. `default_extractor_stack()` must not default to `MilitaryLeader`.

Universal life types are always allowed: `birth`, `death` (only if P570 present), `residence`, `travel` / `arrival` / `departure`, `education`, `marriage`.

## Time

- Window if P569 and P570: `[birth−2, death+5]`.
- Window if P569 and no P570 (living **or** death unknown): `[birth−2, min(now, birth+120)]`. Do **not** assemble an active quality `death`.
- Window if neither: `[-4000, now]`. Never default to 1000–2100 or 1765–1865.
- Parse CE 4-digit years **and** BCE surfaces: `69 BC`, `69 BCE`, `av. J.-C.`, and signed Wikidata years (negative). Do not coerce a year-only value to 1 January.
- Remove Napoleonic clamps from quality extractors and from the mock default when the page is not a Napoleon fixture. `quality` path must not use `1765–1865`.
- `first_year_in_window` must accept years outside 1000–2100 (including negative).

## Places

Resolution order (unchanged product rule):

1. Place in the clause
2. Nearby context (page title, adjacent sentences) — method `context`, lower confidence
3. Known entity (birth place only for `birth`)
4. `place_aliases` + Wikidata P625 of the **place** QID
5. No coords → `timeline_eligible=true`, `map_eligible=false`

The offline gazetteer (Ajaccio, Waterloo, …) is **test/cold-start data**, not an authority. Alexandria, Athens, CERN, Élysée resolve like Waterloo: Wikidata / page coordinates / aliases. Do not map a country label to a capital centroid (no “Egypt = Cairo” unless the clause names Cairo).

## Acceptance panel (tests, fixture, no live network)

No point quota. Fail if `battle+siege > 0` with **no** person-level military/conflict signal.

| Subject | Facets (from that QID / fixture occupations) | Must contain | Must refuse |
|---|---|---|---|
| Marie Curie | scientist | `birth` and (`education` or `publication` or `discovery` or `award`) | `battle`/`siege` (she has no soldier signal) |
| Victor Hugo | writer | `publication` or `exile` or `residence` | battles attached only via “Battle of” links |
| Leonardo da Vinci | artist + scientist + engineer | at least one of `creation`/`publication` and at least one of `education`/`discovery`/`invention` | a battle-only trace |
| Christopher Columbus | explorer | `arrival` or `departure` or travel | wars outside participation |
| Cleopatra | ruler + antiquity | `office` or `diplomatic` or `residence` with year ≤ 0 | empty result from a 1000–2100 clamp |
| Living public figure (Macron fixture, class `ruler`) | ruler | `office` or `residence`; **zero** active quality `death` | invented death singleton |
| Writer-soldier fixture | writer **and** military | ≥1 military fact (`battle` or service/enlistment) | dropping the military career because writer matched first |
| Napoleon | military + ruler | still may have `battle` **and** `office`/`diplomatic` | not a merge gate; not a density floor |

## Files (implementation)

Change:

- `crates/talaria-sources/src/person_profile.rs` — all facets; merge boosts; military crawl flag from **person signal**, not from primary class
- `crates/talaria-sources/src/plan.rs` — `person_classes()`, catalog queries union
- `crates/talaria-sources/src/extractors/mod.rs` — stack union; no military default
- `crates/talaria-sources/src/seeds.rs` — lifespan including BCE and living
- `crates/talaria-sources/src/wdqs.rs` — already P710/P1344/P607; gate on person signal
- `crates/talaria-api/src/lot_e.rs` / `ingest.rs` — ranking, Battle-of BFS, extractors
- `crates/talaria-quality/src/time_typed.rs` — BCE parse
- `crates/talaria-cosmos/src/mock.rs` — no global 1765–1865 default
- `crates/talaria-judge/src/dump_mine.rs` — drop `year_min: 1765` as a global
- Tests under `talaria-sources` / `talaria-quality` for the panel
- `AGENTS.md` — remove ≥500 Napoleon as a product floor; state person-first ingest

Do not change: quality gates semantics, `map_eligible` without coords, Intuition crate, dump JSONL CLI, coexistence legacy/quality.

## Risks

- Facet union without a participation filter re-imports Hugo’s 56 battles. Participation is mandatory.
- BCE parse that only allows 4-digit negatives will still miss “69 BC”. Surfaces `BC`/`BCE`/`av. J.-C.` are required.
- Living vs missing P570: both skip death assemble; window caps at `now`.
- Gazetteer Napoleonic entries may remain as coordinates for those real places; tests must not use Napoleon as the only passing subject.
