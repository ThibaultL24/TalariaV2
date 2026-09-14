# Baudelaire Core Extractor Expansion Evaluation

**Goal**: Expand TalariaV2 person-pipeline extraction so CORE Wikipedia/Wikidata yields richer timeline + map pins for subjects like Charles Baudelaire (Q501).

**Context**: After PR #36 fixed birth/death pollution bugs and plain-text place parsing, core page yield was still ~15 canonical events / ~6 map pins vs ~40 expected from the FR Wikipedia page alone. This PR addresses the extractor coverage gap.

## Inventory: Expected Events from FR Wikipedia

Based on analysis of https://fr.wikipedia.org/wiki/Charles_Baudelaire, these are the key biographical facts that should become events:

### Education (NEW extractor coverage)
| Year | Event | Place | Expected Type |
|------|-------|-------|---------------|
| 1831 | Inscrit à la pension Delorme, collège royal | Lyon | education |
| 1836 | Inscrit comme pensionnaire | Collège Louis-le-Grand, Paris | education |
| 1836-37 | Concours général prizes (vers latins) | Paris | education |
| 1839 | Renvoyé du lycée | Louis-le-Grand | education |
| 1839 | Baccalauréat | Lycée Saint-Louis | education |

### Travel/Voyages (EXPANDED patterns)
| Year | Event | Place | Expected Type |
|------|-------|-------|---------------|
| 1841 Jun | Ship departs | Bordeaux | departure |
| 1841 Sep | Stopover (fait escale) | Port-Louis, Île Maurice | arrival |
| 1841 Sep | Arrives | Saint-Denis, La Réunion | arrival |
| 1841 Nov | Departs | La Réunion | departure |
| 1842 Feb | Returns | Paris | arrival |
| 1864 Apr | Departs for Belgium | Brussels | departure |

### Residences (EXPANDED patterns)
| Year | Event | Place | Expected Type |
|------|-------|-------|---------------|
| 1842+ | Various domiciles | Paris 4e, 11e, 7e, 9e | residence |
| 1859, 1860, 1865 | Stays with mother | Honfleur | residence |
| 1864-66 | Se fixe à | Bruxelles | residence |
| 1866 Jul | Admis dans la maison de santé | Rue du Dôme, Paris 16e | residence |

### Life Events (NEW patterns)
| Year | Event | Place | Expected Type |
|------|-------|-------|---------------|
| 1866 Mar | Perd connaissance (collapse) | Église Saint-Loup, Namur | health_event |
| 1866 Jul | Ramené à | Paris | arrival |
| 1867 Aug | Burial | Cimetière du Montparnasse | burial |

### Meetings (EXPANDED patterns)
| Year | Event | Person | Expected Type |
|------|-------|--------|---------------|
| 1842 | S'éprend de | Jeanne Duval | meeting |
| 1864-66 | Rend plusieurs visites à | Victor Hugo | meeting |
| 1866 | Meets | Félicien Rops | meeting |

## Changes Made

### 1. New EducationLifeExtractor (`education.rs`)

Handles French education patterns:
- `inscrit à/au`, `s'inscrit`, `est inscrit(e)` → education/enrolled_at
- `suit les cours`, `suivait les cours` → education/studied_at
- `pensionnaire au collège/lycée` → education/boarded_at
- `au collège`, `au lycée`, `collège royal de` → education/studied_at
- `baccalauréat`, `passe son bac` → education/graduated_from
- `renvoyé du lycée/collège` → education/expelled_from
- `concours général`, `accessit` → education/received_prize

Handles burial patterns:
- `inhumé au/à`, `enterré au/à` → burial/buried_at
- `cimetière` → burial/buried_at

Handles health/collapse events:
- `perd connaissance`, `s'effondre` → health_event/collapsed_at
- `admis dans la maison de santé/hôpital` → residence/admitted_to

### 2. Expanded TravelResidenceExtractor (`travel.rs`)

Added French patterns:
- `part pour`, `quitte/quitta` → departure
- `fait/faire escale` → arrival (stopover)
- `se fixe/fixa`, `loge/logea à`, `domicilié(e)` → residence
- `revient/revint/retourne/retourna à`, `ramené/ramenée à` → arrival/returned_to
- `rend visite`, `rend plusieurs visites` → meeting/visited

### 3. Expanded ItineraryExtractor (`itinerary.rs`)

Added motion cues:
- `quitte/quitta/quittent`
- `repart/repartit`
- `fait/fit/faire escale`

### 4. Expanded DeterministicClauseAnalyzer (`analyzer.rs`)

Added patterns to the dense clause analyzer:
- Education: `est inscrit(e)`, `inscrit à/au`, `pensionnaire au`, etc.
- Burial: `inhumé au/à`, `buried at/in`

### 5. Updated Map Locus Types (`gates.rs`)

Added `health_event` to `event_type_is_map_locus()` so collapse events with resolved places can become map pins.

## Expected Yield Improvement

### Before (PR #36 baseline)
- ~15 canonical events from core
- ~6 map_eligible events

### After (this PR)
Expected yield from Baudelaire core page:
- Education events: +5 (Lyon, Louis-le-Grand ×2, Saint-Louis, expulsion)
- Travel events: +6 (Bordeaux, Port-Louis, Saint-Denis, La Réunion departure, return to Paris, Belgium)
- Residence events: +4 (Paris addresses, Honfleur, Bruxelles, rue du Dôme)
- Life events: +2 (Namur collapse, Montparnasse burial)
- Meeting events: +3 (Jeanne Duval, Victor Hugo visits, Félicien Rops)

**Target**: ~35-40 canonical events from core (up from ~15)
**Target map_eligible**: ~20+ (up from ~6)

## Test Coverage

New tests added:
- `french_college_enrollment` - Lyon 1831
- `french_lycee_boarding` - Louis-le-Grand
- `french_expelled_from_school` - 1839 expulsion
- `french_baccalaureat` - Saint-Louis
- `french_burial_at_cemetery` - Montparnasse
- `french_collapse_at_church` - Namur 1866
- `hospital_admission` - Rue du Dôme
- `french_settles_at_brussels` - Se fixe à Bruxelles
- `french_return_to_paris` - Ramené à Paris
- `french_leaves_bordeaux` - Quitte Bordeaux
- `french_stopover_mauritius` - Fait escale à Port-Louis
- `french_visit_victor_hugo` - Rend plusieurs visites

## Remaining Gaps

Publication events (Les Fleurs du mal 1857) are intentionally NOT map_eligible via `event_type_is_map_locus` — they stay on the timeline but don't create map pins. This is by design per AGENTS.md.

Events that may still be missed:
- Very specific locations embedded in long paragraphs
- Events without clear year indicators
- Events where the place is only mentioned via wikilink (requires wikitext parsing)

## PR #36 Regression Protection

The birth/death filters from PR #36 remain effective:
- `keep_extracted_raw()` still blocks birth/death from non-biography pages
- `page_is_subject_biography()` check intact
- Plain-text place parsing still works for "Paris, France" style values

All existing tests continue to pass.
