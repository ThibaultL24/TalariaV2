# SNAC — identités, relations, fonds (lot 1)

Date: 2026-10-18  
Status: **revue d’architecture intégrée — en attente de validation**  
Décisions: lot 1 = SNAC seul; lot 2 (texte Gallica / Wikisource / Europeana IIIF) reporté; destination = relations persistées et API, sans écran.  
Revue: les six corrections bloquantes (ordre d’alignement, bijection, contrat d’identifiants, snapshots, clé d’assertion et `is_current`, transaction) sont intégrées. Les PR se suivent ; ils ne sont pas indépendants.

## Objectif

À partir d’une personne déjà résolue dans Talaria, enregistrer :

- l’identité SNAC qui lui correspond, ou le constat explicite qu’elle est ambiguë, en conflit, scindée ou rejetée ;
- les relations personne–personne et personne–institution **attestées par une assertion SNAC**, dans le sens donné par SNAC ;
- les ressources citées par cette notice, qualifiées selon leur `documentType`, sans jamais les déclarer en texte intégral.

Le gain se mesure en identités alignées, relations courantes exploitables et ressources correctement qualifiées. Le volume de notices brutes n’est pas un succès.

## Hors périmètre

- Lot 2 : passages, IIIF, OCR, faits extraits du texte.
- Écran, graphe visuel, endpoint de parcours (`/graph`, voisins à profondeur N).
- Événement canonique, lieu de présence, coordonnées, `place_resolutions`.
- Fusion d’entités, écrasement d’un QID, choix du premier homonyme, choix d’un identifiant quand deux autorités se contredisent.
- Lecture HTTP de chaque correspondant. Le lot 1 lit la constellation du sujet ; les cibles restent des fiches incomplètes.
- Assertion inverse fabriquée. `direction=in` est une lecture, pas une deuxième ligne.
- Remplacement de FranceArchives, VIAF, ISNI ou IdRef.

## Audit des primitives actuelles

| Primitive | Rôle réel | Usage SNAC |
|---|---|---|
| `entities` | Personne, lieu, organisation. `qid` unique s’il est renseigné. `wikipedia_title` obligatoire. | Sujet Talaria uniquement. Pas d’entité créée pour un correspondant. |
| `entities.authority_ids` | JSON fusionné depuis Wikidata (`bnf`, `viaf`, `isni`, `idref`, `gallica_ark`). Pas de statut de conflit. | Écriture de `snac_ark` **seulement** après alignement certain. Jamais une preuve d’alignement. |
| `ResolvedSubject.known_identifiers` | `Vec<(String, String)>`. Au moment du corpus, `person_ingest` passe `vec![]` alors que le bundle Wikidata vient d’être persisté. | Insuffisant. SNAC reçoit un contrat typé rempli depuis ce bundle déjà en mémoire, ou depuis la ligne `entities` si le bundle mémoire est vide. Pas de second appel Wikidata. |
| `claims` / `soft_claims` / `quality_claims` | Opinion Agora, texte libre, fait orienté événement. | Interdits pour ce graphe. |
| `canonical_events` | Faits acceptés, append-only. | Aucune écriture. |
| `document_snapshots` | Preuve immuable : `UNIQUE (source_type, source_uri, content_hash)`, `revision_id`, `text NOT NULL`. | Un snapshot par constellation lue. `text` = JSON canonique. `revision_id` = version SNAC. Les relations pointent vers ce snapshot. Pas de quatrième table. |
| `corpus_documents` | Identité documentaire durable. `full_text_available`, `access_level`. | Ressources SNAC. `full_text_available = false` toujours. Le type suit `documentType`, pas un fonds par défaut. |
| `document_contributions` | Rôles, dont `institution`. `replace_document_contributions` fait un `DELETE` puis des `INSERT`. | Institution **de conservation** seulement. L’appel reste dans la transaction du harvest : un échec annule le `DELETE`. |
| `entity_document_links` | `about \| by \| mentioned_in`. | Étendu. Colonne `is_current` pour retirer une ressource de l’API sans effacer le corpus. |
| `document_fragments` | Passages. L’endpoint fragments renvoie encore une liste vide. | Interface du lot 2. SNAC n’y écrit pas. |

Il n’existe pas de table de relations entre entités. `entity_relations` n’est pas une copie des `claims`.

## Contrat d’identifiants

SNAC ne lit pas `known_identifiers` tant que ce vecteur est vide. Le harvest reçoit :

```rust
pub struct SubjectAuthorities {
    pub entity_id: Uuid,
    pub qid: Option<String>,
    pub label: String,
    pub birth_year: Option<i32>,
    pub death_year: Option<i32>,
    pub viaf: Option<String>,
    pub isni: Option<String>,
}
```

Remplissage, dans l’ordre, sans HTTP :

1. QID déjà résolu par l’ingestion personne.
2. `wd_meta.authority_bundle` déjà chargé dans `person_ingest` (VIAF P214, ISNI P213).
3. Si ce bundle est absent : `entities.authority_ids` de la ligne persistée.

`known_identifiers` du sujet corpus est rempli avec les mêmes valeurs (`("viaf", …)`, `("isni", …)`). SNAC n’aligne pas sur un vecteur vide.

Normalisation avant comparaison : QID `Q` + chiffres ; VIAF chiffres uniquement ; ISNI 16 caractères sans espaces ni tirets. Une forme vide est une absence, pas une contradiction.

## ARK et `same_as`

Clé canonique d’un ARK : retirer un préfixe `http://n2t.net/`, `https://n2t.net/`, `http(s)://snaccooperative.org/` ou `http(s)://www.snaccooperative.org/`, puis exiger un reste qui commence par `ark:/`, en minuscules. Toute autre forme est rejetée et n’aligne rien.

`same_as` conserve les ensembles et les URI d’origine :

```json
{
  "values": { "wikidata": ["Q517"], "viaf": ["106964661"], "isni": ["0000000121264454"] },
  "raw": ["http://www.wikidata.org/entity/Q517"],
  "internal_conflict": false
}
```

Deux valeurs normalisées distinctes pour un même schéma : `internal_conflict = true`. Ce candidat est `conflict` / `internal_identifier_conflict`. On n’en choisit pas une.

## Modèle

Migrations après `042`. Pas de `TRUNCATE`. Pas d’écriture dans `canonical_events`.

### PR 1 — `043_snac_constellations.sql`

```sql
CREATE TABLE snac_constellations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    snac_id TEXT,
    ark TEXT NOT NULL,
    snac_version TEXT,
    entity_type TEXT NOT NULL
        CHECK (entity_type IN ('person', 'corporateBody', 'family')),
    label TEXT NOT NULL,
    record_status TEXT NOT NULL
        CHECK (record_status IN ('stub', 'complete', 'split', 'withdrawn')),
    same_as JSONB NOT NULL DEFAULT '{"values":{},"raw":[],"internal_conflict":false}'::jsonb,
    exist_dates JSONB NOT NULL DEFAULT '{"kind":"unknown"}'::jsonb,
    snapshot_id UUID REFERENCES document_snapshots(id) ON DELETE SET NULL,
    raw_json JSONB NOT NULL DEFAULT '{}'::jsonb,
    source_url TEXT NOT NULL,
    retrieved_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (ark),
    CHECK (exist_dates->>'kind' IN ('exact', 'range', 'approx', 'unknown'))
);

CREATE UNIQUE INDEX uq_snac_constellations_snac_id
    ON snac_constellations (snac_id)
    WHERE snac_id IS NOT NULL;

CREATE TABLE entity_identity_links (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_id UUID NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    constellation_id UUID NOT NULL REFERENCES snac_constellations(id) ON DELETE CASCADE,
    status TEXT NOT NULL
        CHECK (status IN ('aligned', 'ambiguous', 'conflict', 'rejected')),
    method TEXT NOT NULL
        CHECK (method IN (
            'qid_same_as',
            'viaf_same_as',
            'isni_same_as',
            'viaf_and_isni',
            'conflicting_qid',
            'conflicting_authority',
            'internal_identifier_conflict',
            'multiple_certain_matches',
            'constellation_claimed',
            'split_identity',
            'label_and_years',
            'label_only'
        )),
    evidence JSONB NOT NULL DEFAULT '{}'::jsonb,
    retrieved_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (entity_id, constellation_id)
);

CREATE UNIQUE INDEX uq_entity_identity_links_one_aligned_entity
    ON entity_identity_links (entity_id)
    WHERE status = 'aligned';

CREATE UNIQUE INDEX uq_entity_identity_links_one_aligned_constellation
    ON entity_identity_links (constellation_id)
    WHERE status = 'aligned';

CREATE INDEX idx_entity_identity_links_entity_status
    ON entity_identity_links (entity_id, status);
```

Les deux index partiels donnent la bijection des alignements certains. Plusieurs lignes `ambiguous` restent possibles. Les index ne décident pas : le conflit est calculé en mémoire, puis écrit.

`record_status` : `complete` après un `read` d’une seule constellation ; `stub` pour une cible non lue ; `split` si le `read` renvoie plusieurs constellations ; `withdrawn` si SNAC annonce une notice retirée. `raw_json` est le dernier état observé. La preuve historique est `document_snapshots`, pas `raw_json`.

Snapshot d’une lecture complète : `source_type = snac`, `source_uri` = URL canonique de l’ARK, `source_identifier` = `snac_id`, `revision_id` = version SNAC, `content_hash` = SHA-256 du JSON canonique, `text` = ce JSON. `INSERT … ON CONFLICT DO NOTHING`. Jamais de `UPDATE` du texte.

`exist_dates` et, plus bas, `dates_json` : `kind` limité à `exact | range | approx | unknown`. En Rust, `precision` n’est acceptée que parmi `day | month | year`, et seulement si `kind` n’est pas `unknown`. Sinon `{ "kind": "unknown", "surface": "<texte d’origine>" }`. Ce temps n’est pas un événement.

### PR 2 — `044_entity_relations.sql`

```sql
CREATE TABLE entity_relations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    subject_constellation_id UUID NOT NULL
        REFERENCES snac_constellations(id) ON DELETE CASCADE,
    object_constellation_id UUID NOT NULL
        REFERENCES snac_constellations(id) ON DELETE CASCADE,
    predicate TEXT NOT NULL
        CHECK (predicate IN (
            'corresponded_with', 'associated_with', 'acquaintance_of',
            'parent_of', 'child_of', 'sibling_of', 'spouse_of',
            'member_of', 'has_member', 'founder_of', 'founded_by',
            'unmapped'
        )),
    source_predicate TEXT NOT NULL,
    source_assertion_key TEXT NOT NULL,
    snac_relation_id TEXT,
    assertion_text TEXT,
    dates_json JSONB NOT NULL DEFAULT '{"kind":"unknown"}'::jsonb,
    validation_status TEXT NOT NULL
        CHECK (validation_status IN ('accepted', 'needs_review')),
    is_current BOOLEAN NOT NULL DEFAULT true,
    source_kind TEXT NOT NULL DEFAULT 'snac' CHECK (source_kind = 'snac'),
    source_url TEXT NOT NULL,
    snapshot_id UUID NOT NULL REFERENCES document_snapshots(id),
    content_hash TEXT NOT NULL,
    snac_version TEXT NOT NULL,
    retrieved_at TIMESTAMPTZ NOT NULL,
    superseded_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (subject_constellation_id <> object_constellation_id),
    CHECK (dates_json->>'kind' IN ('exact', 'range', 'approx', 'unknown')),
    CHECK (
        (validation_status = 'accepted' AND predicate <> 'unmapped')
        OR (validation_status = 'needs_review' AND predicate = 'unmapped')
    )
);

CREATE UNIQUE INDEX uq_entity_relations_current_assertion
    ON entity_relations (subject_constellation_id, source_assertion_key)
    WHERE is_current;

CREATE INDEX idx_entity_relations_subject_current
    ON entity_relations (subject_constellation_id, predicate)
    WHERE validation_status = 'accepted' AND is_current;

CREATE INDEX idx_entity_relations_object_current
    ON entity_relations (object_constellation_id, predicate)
    WHERE validation_status = 'accepted' AND is_current;
```

`source_assertion_key` : `snac:{id}` lorsque l’assertion a un identifiant natif. Sinon SHA-256 d’un JSON canonique trié : ARK sujet, ARK objet, `source_predicate`, surface de dates, texte d’assertion normalisé (espaces repliés, casse d’origine conservée). Deux assertions sans id entre les mêmes personnes ne s’écrasent pas si la période ou le texte diffère.

Même clé et même `content_hash` : pas de nouvelle ligne. Même clé et hash différent : la ligne courante passe `is_current = false`, `superseded_at` renseigné, puis insertion de la nouvelle version. L’ancienne ligne reste.

Synchronisation : seulement après un `read` complet réussi d’**une** constellation. Les assertions courantes du sujet absentes de ce harvest passent `is_current = false`. Elles ne sont pas supprimées. Un `read` en échec, un timeout, un corps tronqué ou une transaction annulée ne change aucun `is_current`.

`corresponded_with` seulement si `source_predicate = correspondedWith`. `associatedWith` reste `associated_with`. Deux noms dans une même ressource ne créent pas de ligne.

| `relations[].type` | Prédicat | Objet admis |
|---|---|---|
| `correspondedWith` | `corresponded_with` | `person`, `family` |
| `associatedWith` | `associated_with` | `person`, `corporateBody`, `family` |
| `acquaintanceOf` | `acquaintance_of` | `person` |
| `parentOf` | `parent_of` | `person` |
| `childOf` | `child_of` | `person` |
| `siblingOf` | `sibling_of` | `person` |
| `spouseOf` | `spouse_of` | `person` |
| `memberOf` | `member_of` | `corporateBody` |
| `hasMember` | `has_member` | `person` |
| `founderOf` | `founder_of` | `corporateBody` |
| `foundedBy` | `founded_by` | `person`, `corporateBody` |
| autre, ou objet hors table | `unmapped` / `needs_review` | conservé, absent de l’API |

Le sens stocké est le sens SNAC (sujet de la constellation lue → cible). Aucune ligne réciproque n’est ajoutée.

`altType` est ignoré.

### PR 3 — ressources et transaction

```sql
ALTER TABLE entity_document_links
    DROP CONSTRAINT IF EXISTS entity_document_links_relation_check;
ALTER TABLE entity_document_links
    ADD CONSTRAINT entity_document_links_relation_check
    CHECK (relation IN (
        'about', 'by', 'mentioned_in', 'contributor_of', 'editor_of'
    ));

ALTER TABLE entity_document_links
    ADD COLUMN IF NOT EXISTS is_current BOOLEAN NOT NULL DEFAULT true;

CREATE INDEX IF NOT EXISTS idx_entity_document_links_current
    ON entity_document_links (entity_id, relation)
    WHERE is_current;

CREATE TABLE corpus_document_matches (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    corpus_document_id UUID NOT NULL REFERENCES corpus_documents(id) ON DELETE CASCADE,
    matched_document_id UUID NOT NULL REFERENCES corpus_documents(id) ON DELETE CASCADE,
    method TEXT NOT NULL CHECK (method IN ('canonical_url', 'ark')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (corpus_document_id, matched_document_id, method),
    CHECK (corpus_document_id <> matched_document_id)
);

CREATE INDEX idx_corpus_document_matches_matched
    ON corpus_document_matches (matched_document_id);
```

`corpus_document_matches` est la correspondance requêtable. Le champ API `known_document_ids` en est une projection. Il n’est pas la donnée source.

| Rôle SNAC | Lien |
|---|---|
| `creatorOf` | `by` |
| `referencedIn` | `mentioned_in` |
| `contributorOf` | `contributor_of` |
| `editorOf` | `editor_of` |
| autre | ressource non liée ; compteur `snac_resources_role_unmapped` |

`documentType` SNAC, après passage en minuscules, décide du type Talaria :

| Forme déclarée | `corpus_documents.document_type` | Servi par `/archival-collections` |
|---|---|---|
| contient `fonds`, `collection`, `recordgrp`, `record group`, `series` | `archival_collection` | oui |
| contient `item`, `letter`, `manuscript`, `document` | `manuscript` | non |
| absent, vide, ou autre | `snac_resource` | non |

Une ressource qui n’est pas un fonds est conservée. Elle n’est pas présentée comme un fonds. `full_text_available = false` et `access_level = metadata_only` dans tous les cas. `academic_status = catalog_record`.

Institution **de conservation** : `document_contributions.role = institution` lorsque la resource relation désigne le dépôt. Une organisation seulement liée à la personne (`memberOf`, `associatedWith`) reste une `entity_relations`. Elle n’est pas recopiée comme dépôt. `entity_id` de la contribution seulement si cette institution a déjà un lien `aligned`.

Même règle de présence que les relations : un harvest complet réussi marque `is_current = false` les liens SNAC du sujet absents de cette lecture. Pas de `DELETE` du `corpus_documents`. `replace_document_contributions` s’exécute dans la même transaction.

Bornes, dépassement compté et non silencieux :

- 200 relations par harvest (`snac_relations_truncated`) ;
- 100 ressources (`snac_resources_truncated`) ;
- description 8 000 caractères (`snac_descriptions_truncated`) ;
- corps HTTP supérieur à 2 000 000 octets : erreur, aucune synchronisation.

L’ordre de troncature est l’identifiant SNAC croissant, puis l’ARK.

## Alignement

Décision entière en mémoire, avant toute écriture.

Pour chaque candidat, dans cet ordre :

1. Conflit interne SNAC (deux valeurs d’un même schéma) → `conflict` / `internal_identifier_conflict`. Stop.
2. Si SNAC a au moins un QID : s’il diffère du QID Talaria, ou s’il y en a plusieurs → `conflict` / `conflicting_qid`. Stop. S’il est égal → candidat certain / `qid_same_as`, puis test d’autorité ci-dessous.
3. VIAF et ISNI, seulement si l’étape 2 n’a pas déjà conclu à un conflit de QID. Les deux côtés ont une valeur et les ensembles sont disjoints → `conflict` / `conflicting_authority`. Stop. Cela couvre un VIAF concordant avec un ISNI incompatible, et un QID concordant avec une autorité incompatible.
4. Sinon, si pas encore certain : intersection VIAF et ISNI → `viaf_and_isni` ; VIAF seul → `viaf_same_as` ; ISNI seul → `isni_same_as`.
5. Sinon nom et années compatibles → `ambiguous` / `label_and_years`.
6. Sinon nom seul → `ambiguous` / `label_only`.

Une absence d’ISNI ou de VIAF n’est pas un conflit. Une valeur présente des deux côtés et différente l’est.

Puis, encore en mémoire :

- deux candidats certains pour la même entité → tous `conflict` / `multiple_certain_matches`, zéro `aligned` ;
- `read` multiple ou notice scindée → `conflict` / `split_identity`, `record_status = split`, zéro relation, zéro ressource ;
- constellation déjà `aligned` à une autre entité → le nouveau lien est `conflict` / `constellation_claimed`. L’alignement existant n’est pas volé ;
- notice retirée → `record_status = withdrawn`, pas de promotion, pas de désactivation des relations.

Transitions automatiques :

| De | Vers | Condition |
|---|---|---|
| `ambiguous` | `aligned` | une preuve certaine apparaît, et la bijection est libre |
| `ambiguous` | `conflict` | QID, autorité, scission ou double certitude |
| `aligned` | `conflict` | le QID SNAC change, une autorité se contredit, ou la constellation est réclamée |
| `conflict` | `aligned` | uniquement si la nouvelle preuve est `qid_same_as` et l’ancien motif était `conflicting_qid`, `label_and_years` ou `label_only` |
| `rejected` | — | aucune. `rejected` est réservé à un acte explicite hors pipeline. Le lot 1 ne l’écrit pas et ne le retire pas |

`aligned` ne redevient pas `ambiguous` sur un écart de libellé.

`authority_ids.snac_ark` est écrit seulement si le statut final est `aligned`. Un conflit n’efface pas la clé. L’API lit `entity_identity_links`, pas cette clé.

## HTTP

`PUT https://api.snaccooperative.org/`, JSON, sans clé. User-Agent Talaria. Timeout 30 s. `429` → `RateLimited`, et ce harvest ne synchronise pas.

Budget par personne : au plus 5 résultats de `search`, au plus **3** commandes `read`. Ces 3 incluent la lecture définitive. Une constellation déjà lue est réutilisée depuis le cache du harvest (clé = id SNAC, sinon ARK). On lit d’abord les candidats dont le résumé porte déjà un `sameAs`, afin que la lecture utile consomme le budget avant les lectures exploratoires.

Sans QID Talaria et sans VIAF ni ISNI, aucun `aligned` : les homonymes restent `ambiguous`. Pas de recherche Wikidata de rattrapage.

## Transaction

```
HTTP et normalisation hors transaction
        ↓
décision d'alignement en mémoire
        ↓
BEGIN
  pg_advisory_xact_lock(entité)
  relire les liens aligned
  réappliquer la bijection
  snapshot immuable
  upsert constellations
  upsert liens d'identité
  upsert relations et marquer les absentes (seulement si read complet réussi)
  upsert ressources, contributions, correspondances, liens
  métriques du run
COMMIT
```

Le verrou est tenu pour toute la transaction, donc pour une entité à la fois. Deux entités qui visent la même constellation : la seconde reçoit la violation d’index ou voit le lien déjà pris, annule, et réécrit son lien en `constellation_claimed`. Aucun état `aligned` des deux côtés.

`replace_document_contributions` utilise la même transaction. Son `DELETE` n’est visible qu’après `COMMIT`.

La fonction de production `persist_snac_harvest` apparaît au PR 3. L’ingestion personne ne l’appelle qu’au PR 4. Avant cela, aucun chemin live n’écrit une partie du graphe.

## Connecteur

`SourceKind::Snac` (`snac`). Live, sinon stub. `discover` ne projette que les ressources qualifiées `archival_collection` du sujet aligné, et renvoie une page vide avant le PR 3. `fetch` renvoie la notice ; la persistance laisse `full_text_available = false`.

```rust
pub async fn harvest(&self, subject: &SubjectAuthorities) -> Result<SnacHarvest, ConnectorError>
```

`plan_sources` : priorité 45, toutes les classes, `max_documents` 20 pour les fonds servis. Le stub ne crée ni lien ni relation.

Capabilities : alignement et identifiants oui, texte intégral non, IIIF non, palier institutionnel, types `authority_record` et `archival_collection`.

Les chemins JSON (`relations`, `resourceRelations`, `sameAs`, version, scission) sont figés sur des réponses **capturées**, pas sur un schéma imaginé. Le PR 1 commence par enregistrer `search` et `read` pour Napoléon, Hugo et Curie. Le normaliseur de parse n’est pas figé avant ces fichiers. Les tests de décision d’alignement peuvent utiliser des fixtures `synthetic`, nommées ainsi, distinctes des captures.

## API

Style de `GET /api/v1/entities/{entity_id}/documents` : `limit` 1..200, curseur opaque, `404` si l’entité n’existe pas, `200` et `items: []` si aucun lien SNAC. Les deux chemins sont ajoutés à `docs/openapi-v3.yml` et au miroir `web/src/lib/schemas/openapi-v3.yml`.

Filtre permanent des relations :

```sql
validation_status = 'accepted' AND is_current
```

### `GET /api/v1/entities/{entity_id}/relations`

`direction` = `out` (défaut) ou `in`. Pas de `both`. `out` : la constellation alignée est le sujet SNAC. `in` : elle est l’objet SNAC. Les deux extrémités sont toujours renvoyées. `direction` ne sert qu’à dire laquelle est l’entité demandée.

```json
{
  "entity_id": "…",
  "alignment": { "status": "aligned", "method": "qid_same_as", "snac_ark": "ark:/99166/…" },
  "items": [{
    "id": "…",
    "predicate": "member_of",
    "direction": "out",
    "subject": { "entity_id": "…", "snac_ark": "ark:/99166/…", "label": "Personne A", "entity_type": "person" },
    "object": { "entity_id": null, "snac_ark": "ark:/99166/…", "label": "Institution B", "entity_type": "corporateBody", "record_status": "stub" },
    "source_predicate": "memberOf",
    "source_assertion_key": "snac:123",
    "assertion_text": "…",
    "dates": { "kind": "unknown", "surface": "1796" },
    "source": { "kind": "snac", "url": "https://snaccooperative.org/ark:/99166/…", "version": "4", "snapshot_id": "…", "content_hash": "…" },
    "retrieved_at": "2026-10-18T00:00:00Z"
  }],
  "next_cursor": null
}
```

Curseur `(predicate, id)`.

### `GET /api/v1/entities/{entity_id}/archival-collections`

Uniquement `document_type = archival_collection`, lien courant, rôles `by`, `mentioned_in`, `contributor_of`, `editor_of`. `relation` est le rôle du **sujet** envers la ressource. `holding_institution` est le dépôt. Il n’est pas une organisation seulement associée à la personne.

```json
{
  "entity_id": "…",
  "items": [{
    "id": "…",
    "title": "…",
    "document_type": "archival_collection",
    "relation": "by",
    "canonical_url": "https://…",
    "access": { "level": "metadata_only", "full_text_available": false },
    "holding_institution": { "name": "…", "snac_ark": null, "entity_id": null },
    "known_document_ids": [],
    "source": { "kind": "snac", "external_id": "…" },
    "retrieved_at": "…"
  }],
  "next_cursor": null
}
```

`full_text_available: false` est obligatoire. Pas de fragments pour ces lignes.

## Métriques

Dans `source_discovery_runs.metrics_json` :

| Clé | Sens |
|---|---|
| `snac_notices` | candidats de `search` |
| `snac_aligned` | 0 ou 1 |
| `snac_ambiguous` | ambigus |
| `snac_conflict` | conflits, scissions comprises |
| `snac_relations_accepted` | courantes et acceptées |
| `snac_relations_needs_review` | `unmapped` |
| `snac_relations_retired` | passées `is_current = false` lors de ce harvest |
| `snac_holdings` | `archival_collection` écrits |
| `snac_resources_other` | manuscrits ou `snac_resource` |
| `snac_holdings_linked_known_docs` | lignes dans `corpus_document_matches` |
| `snac_holdings_full_text` | toujours 0 |
| `snac_relations_truncated`, `snac_resources_truncated`, `snac_descriptions_truncated` | bornes atteintes |
| `snac_read_budget_exhausted` | 0 ou 1 |

## Lot 2

Inchangé. Le lot 2 lira les documents `full_text_available = true` de Gallica, Wikisource et Europeana, puis `document_fragments`. Les lignes SNAC sont exclues par le drapeau. Aucune table de passages nouvelle dans ce lot.

## Tests

Décision, sans réseau, fixtures `synthetic` :

- QID différent et VIAF identique → `conflict`, pas `aligned` ;
- QID identique et ISNI incompatible → `conflict` ;
- VIAF identique, ISNI absent → `aligned` / `viaf_same_as` ;
- deux QID internes → `internal_identifier_conflict` ;
- deux entités certaines sur une constellation → une seule peut être `aligned`, l’autre `constellation_claimed` ;
- deux assertions sans id natif, textes différents → deux clés ;
- second harvest sans une relation → cette relation `is_current = false`, toujours lisible en base, absente de l’API ;
- `read` en échec après des lignes existantes → aucune désactivation ;
- `read` tableau de plusieurs constellations → `split`, zéro relation servie ;
- transaction interrompue après les relations et avant les fonds → aucune des écritures du harvest n’est visible ;
- deux harvests concurrents de deux entités vers la même constellation → un seul `aligned` ;
- ARK `https://n2t.net/ark:/99166/…` et `ark:/99166/…` → même clé ;
- date illisible → `kind = unknown` ; `kind` hors liste refusé ;
- fonds : `full_text_available` faux ; un `documentType` de lettre n’apparaît pas dans `/archival-collections`.

Parse : fixtures `captured` de vrais `search` et `read` (Napoléon, Hugo, Curie) avant de figer les chemins JSON. Napoléon couvre un QID concordant ; Hugo un homonyme ambigu ; Curie un alignement VIAF ou ISNI sans QID SNAC.

API : `sqlx::test(migrations = "../../migrations")`, les deux extrémités présentes, `needs_review` et `is_current = false` invisibles, `404` si l’entité manque.

## Découpage

Chaîne de migrations. Le PR *n* suppose le schéma du PR *n − 1* appliqué. Aucun n’est un produit livré seul, sauf comme incrément testé.

| PR | Contenu | Condition de sortie |
|---|---|---|
| 1 — Identités | Capture réelle, tables 043, ARK, `same_as` en ensembles, alignement, conflits, versions, `SubjectAuthorities` | Aucun faux `aligned`, y compris QID contre VIAF et deux entités pour une constellation |
| 2 — Relations | Table 044, prédicats, `source_assertion_key`, snapshots, `is_current` | Rejouer n’ajoute pas de doublon ; une assertion disparue d’un harvest réussi n’est plus courante et n’est pas détruite |
| 3 — Ressources | Migration 045, qualification par `documentType`, dépôt distinct de l’organisation liée, `corpus_document_matches`, `persist_snac_harvest` transactionnel | Aucun faux texte intégral ; une lettre n’est pas un fonds ; un échec au milieu n’efface aucune contribution |
| 4 — API | Routes, OpenAPI, `plan_sources`, branchement sur le bundle déjà chargé, budgets, métriques | Napoléon, Hugo et Curie mesurés sur fixtures capturées ; un essai live reste un contrôle manuel hors `cargo test` |

## Index

- `UNIQUE (ark)` ; `snac_id` unique seulement s’il est présent.
- Un `aligned` par entité et un `aligned` par constellation.
- Relations courantes indexées dans les deux sens.
- Correspondances documentaires indexées dans les deux sens.
- Corpus : les index `(source_kind, external_id)` existants suffisent.
