# Prompt Cursor — Enrichissement documentaire et corpus historique de Talaria

> Prompt adapté au **monorepo TalariaV2 actuel** (Rust / Axum / sqlx / Postgres+PostGIS + React/Vite/MapLibre).  
> Remplace toute version Rails/Gemfile/jobs ActiveJob/WebMock.  
> À coller dans Cursor pour conception + implémentation production.

---

Tu travailles sur **Talaria Engine (TalariaV2)**, un produit historique destiné à la production.

## Stack réelle (ne pas inventer Rails)

| Couche | Réalité |
|--------|---------|
| Backend | **Rust workspace** — crates sous `crates/` ; binaire CLI/API `talaria` (`talaria-api`) |
| HTTP | **Axum** sur `:8080` |
| DB | **Postgres 16 + PostGIS** via `docker compose` (port hôte **5433**), migrations **sqlx embarquées** dans `talaria-store` |
| Front | **React + Vite + MapLibre** dans `web/` (même dépôt, proxy `/api` → `:8080`) |
| Dumps froids | `TALARIA_DATA_ROOT` (ex. `/mnt/wiki-dump`) — hors Postgres |
| NLP | Sidecar Python COSMOS optionnel ; mock `cosmos-extract --mock` en dev/CI |

**Il n’y a plus de Rails.** Pas de `Gemfile`, pas de modèles ActiveRecord, pas d’ActiveJob. Les « jobs » sont des **commandes CLI** `cargo run -p talaria-api -- <cmd>` (éventuellement orchestrées plus tard). Les tests réseau utilisent des **fixtures figées** (`crates/talaria-sources/tests/`, `fixtures/`) — **jamais** le réseau en CI.

Le pipeline actuel ingère déjà Wikidata/Wikipedia (dump-first + connecteurs live optionnels) et produit timeline + GeoJSON. Il existe déjà un socle multi-sources quality (Lot A/B/E) avec stubs BnF/Gallica/Europeana/etc.

---

## Mission

Conçois puis implémente, dans **ce** dépôt (workspace ouvert), une architecture d’ingestion multi-sources sérieuse permettant d’enrichir une personne ou un sujet historique avec :

- travaux universitaires, thèses et articles d’historiens ;
- controverses historiographiques, énigmes et théories, **sans les confondre avec des faits établis** ;
- bibliographie normalisée (ISBN/DOI/identifiants d’autorité) ;
- sources primaires numérisées : livres, manuscrits, correspondances, presse ancienne et archives ;
- médias documentaires : images, audio, vidéo, avec droits et conditions d’intégration explicites ;
- fragments de corpus citables : page, zone OCR, passage, timecode ou canvas IIIF.

Ne construis **pas** un agrégateur de liens. Construis une **couche documentaire traçable, versionnée et exploitable** par :

- les faits culturels → `canonical_events` / `quality_claims` / `event_candidates` (`pipeline='quality'`) ;
- les soft claims Explorer → `soft_claims` + `soft_claim_evidence` ;
- la lane Intuition → table `claims` (`exportable=true`) pour avis / théories / débats **uniquement** ;
- les projections narratives (`narrative_dossier`) et, plus tard, Agora/Intuition — **sans** stocker le corpus culturel dans Intuition.

---

## Avant toute modification

1. Inspecte : `AGENTS.md`, `README.md`, `docs/superpowers/specs/2026-08-06-talaria-implementation-summary.md`, `Cargo.toml` / workspace, `migrations/`, crates `talaria-sources`, `talaria-quality`, `talaria-store`, `talaria-api` (CLI + routes), `web/`, fixtures, tests.
2. Décris brièvement l’architecture existante.
3. Fournis une **table de correspondance** modèle cible ↔ modèle réel (ci-dessous comme point de départ — affine-la après lecture) :

| Concept cible (prompt) | Équivalent actuel TalariaV2 | Notes |
|------------------------|-----------------------------|-------|
| `SourceProvider` / registry | `SourceRegistry`, `ConnectorRegistration`, `SourceKind`, `SourceCapabilities` | Étendre enums (`theses_fr`, `hal`, `crossref`, …) ; stubs déjà présents pour BnF/Gallica/Europeana/… |
| `SourceConnector` | trait `SourceConnector` (`discover` / `fetch` / `healthcheck`) | Adapter ; ajouter `normalize` côté service, pas dans le trait si déjà séparé |
| Document découvert | `discovered_documents` + DTO `DiscoveredDocument` | Rank / fetch_status déjà là |
| Document immuable / snapshot | `raw_documents`, `document_snapshots`, `document_fragments` | Fragments aujourd’hui `sentence`/`clause` — **étendre** kinds (page, ocr_region, iiif_canvas, audio/video_segment) sans casser quality |
| Run d’ingestion | `source_discovery_runs` | Idempotence + métriques JSON à enrichir |
| Claim culturel / assertion | `event_candidates` → `quality_claims` → `canonical_events` (`pipeline='quality'`) | Append-only + supersession |
| Soft claim / anecdote | `soft_claims`, `soft_claim_evidence`, `soft_claim_relations` | Explorer ; pas Intuition |
| Opinion / débat exportable | `claims` (005) | **Intuition only** — ne pas y mettre le corpus culturel |
| Evidence événement | `event_evidence` | Relier aussi fragments documentaires |
| Place | `place_geocodes`, `place_aliases`, `place_resolutions`, entités lieu | Jamais string seule comme vérité |
| Temps typé | `time_json` / typed time quality | Pas de texte libre ; pas année→1er janvier |
| Historiographie / Agora | **Absent ou partiel** | À ajouter (positions / supports) en s’appuyant sur `soft_claims` ou tables dédiées — ne pas réutiliser `claims` Intuition pour le corpus |
| Rails Job / Sidekiq | CLI `ingest-quality`, futurs `corpus-ingest` | Pas d’ActiveJob |
| OpenAPI Rails | Routes Axum + schémas front Zod si besoin | Documenter contrats JSON |

4. Ne duplique aucun concept existant. **Étends** `talaria-sources` / migrations additives (`015+`) ; rebuild après migration (`sqlx` embed).
5. Vérifie l’état Git ; préserve les changements non liés.
6. Présente un plan en petites étapes migrables, puis implémente le **premier lot vertical** si le dépôt le permet.

---

## Invariants Talaria non négociables

- `canonical_events` quality : **append-only** ; corrections par **supersession** uniquement.
- Legacy (`pipeline='legacy'`) **jamais** auto-requalifié en quality.
- Temps **strictement typé** (`exact` / `range` / `approx` / `unknown` / year/month…) — jamais texte libre ; jamais coercion année → 1 jan.
- Un lieu est une **référence** (entité / résolution / géocode), pas une simple chaîne comme vérité.
- Toute donnée Wikidata conserve QID, PID si applicable, date de récupération et révision si dispo.
- Aucun claim accepté (quality ou soft) **sans** au moins une preuve reliée à une source / fragment / snapshot.
- Une ressource découverte ≠ preuve de vérité ≠ événement.
- Un LLM ne crée **jamais** directement un claim accepté, un event ou une narration publiée.
- Tout extrait rattache un **document figé** (`document_snapshots`) + pointeur reproductible.
- Faits / interprétations historiographiques / hypothèses spéculatives : **typés séparément** (et opinions Intuition uniquement dans `claims` exportable).
- Sources supplémentaires **renforcent** une occurrence (`fingerprint` / `occurrence_key`) ; elles ne créent pas de doublons carte.
- Stubs ≠ intégrations : annoncer maturité via `source-status` / `connector-report` seulement.

---

## Sources à intégrer et ordre de priorité

Connecteurs **indépendants** derrière `SourceConnector` (ou extension claire). Le domaine ne dépend d’aucun format fournisseur.

### Lot A — France, haute valeur

- **theses.fr / ABES** — NNT, statut (soutenue vs en cours), titres, résumés, disciplines, RAMEAU, auteurs/directeurs/jury + PPN, établissement, date, langue, URL, accessibilité plein texte. Thèses en cours = ressource documentaire seulement, **pas** autorité équivalente.
- **HAL / HAL-SHS** — halId, DOI, auteurs, ORCID/IdHAL, revue, type, dates, résumé, mots-clés, URI texte, licence, version.
- **BnF data + Gallica** — autorités, notices, ISBN/ARK, numérisés, presse, manuscrits ; SRU/OAI-PMH, API document Gallica, IIIF. Conserver ARK, droits, OCR, manifest IIIF, URL canonique. (Enums `Bnf`/`Gallica` déjà stubbés.)
- **Sudoc + IdRef / ABES** — PPN, ISBN, éditions, auteurs, sujets ; catalogue ≠ preuve du contenu.
- **OpenEdition** — OAI-PMH ; distinguer types éditoriaux / statut revue.
- **Persée** — interfaces / datasets **officiels uniquement** ; pas de scrape HTML. (`Persee` déjà dans `SourceKind`.)

### Lot B — International

- **Crossref**, **OpenAlex**, **Europeana**, **Internet Archive / Open Library**, **VIAF / ISNI / ORCID / ROR** — mêmes règles que le prompt métier d’origine (DOI, alignement d’identité, dumps IA pour le volume, etc.).

### Presse et audiovisuel

- Privilégier Gallica/BnF, Europeana, BN OAI/SRU/IIIF.
- Connecteurs génériques configurables `OaiPmhProvider` / `IiifProvider` + **liste blanche** institutionnelle.
- INA : notice/URL si conditions ok — **jamais** aspirer/réhéberger sans API/accord.
- YouTube / plateformes grand public : jamais source historique de premier rang ; au plus `media_reference` si producteur/programme/date/chaîne/droits identifiés — **pas** de téléchargement/découpe/réhébergement.

---

## Architecture cible (noms adaptés Rust)

Adapte les noms au dépôt ; conserve les responsabilités.

### 1. Registre des fournisseurs

Étendre `SourceKind` + `SourceCapabilities` + `SourceRegistry` :

- `key` enum stricte : ajouter au minimum `ThesesFr`, `Hal`, `Crossref`, `OpenAlex`, `OpenEdition`, `Sudoc`, … (en plus de BnF, Gallica, Europeana, …)
- `authority_tier` : `institutional | academic_publisher | scholarly_index | heritage_aggregator | community_catalog`
- capabilities typées déjà amorcées (`discovery`, `metadata`, …) — étendre : `full_text`, `ocr`, `iiif`, `audiovisual`, `authority_alignment`
- rate limit, auth, licence par défaut, `connector_version` (déjà sur le trait)

Pas de config fourre-tout texte libre : enums + JSON Schema validé ou tables normalisées cohérentes avec sqlx.

### 2. Document source immuable

Étendre ou superposer proprement à `raw_documents` / `document_snapshots` / `discovered_documents` :

- UUID ; provider (`source_kind`) ; identifiant fournisseur ; URL canonique ; `document_type` strict (étendre `DocumentType`)
- titres, langue, contributeurs, éditeur/institution
- dates typées (réutiliser le modèle typed time quality)
- table **`document_identifiers`** : ISBN-10/13, DOI, ARK, PPN, NNT, OCLC/OLID… unicité `(scheme, normalized_value)` quand sûre
- `academic_status` : `peer_reviewed | doctoral_defended | academic_unreviewed | primary_source | catalog_record | unknown`
- accès / plein texte / manifest IIIF
- droits : URI, valeur normalisée, titulaire, `open | restricted | metadata_only | unknown`
- checksum payload, `retrieved_at`, révision/ETag/Last-Modified, version connecteur
- **pas** de binaire distant recopié par défaut

Nouvelle révision distante → **nouveau snapshot** (comme `document_snapshots` UNIQUE sur hash) ; jamais écraser une preuve déjà citée.

### 3. Contributions et sujets

Tables additives : `document_contributions`, `document_subjects`, `document_relations` (rôles distincts : auteur historique vs scientifique, directeur de thèse, réalisateur, personne décrite, etc.).

### 4. Fragments et preuves

Étendre `document_fragments.fragment_kind` : `text_span | page | ocr_region | iiif_canvas | audio_segment | video_segment` (+ conserver `sentence`/`clause` pour quality).

Texte stocké seulement si droits OK ; sinon empreinte / courte citation / pointeur externe.

`AssertionCandidate` : réutiliser / aligner `event_candidates` + chemin soft claim ; après gates/revue → `quality_claims` / `soft_claims` / `claims` Intuition selon le **type épistémique**. Jamais coller le fragment dans un champ description fourre-tout.

### 5. Historiographie

Modèle explicite (nouvelles tables ou extension `soft_claims`) :

- `HistoriographicalPosition`, `PositionSupport`, `ClaimConflict`, `ReviewDecision`
- Consensus **jamais** = compte de docs/citations
- Forme : « X soutient Y dans Z », jamais « Y est vrai »
- Export débats → Intuition `claims` seulement quand `exportable` et typé opinion

### Interface connecteurs (Rust)

S’appuyer sur le trait existant ; pattern de pipeline :

```text
Discover → Fetch → Snapshot → Normalize → Align → Rank
  → Extract fragments → Propose assertions → Review/consolidate → Project
```

Exemple d’extension (illustratif — adapter au code réel) :

```rust
#[async_trait]
pub trait SourceConnector: Send + Sync {
    fn source_kind(&self) -> SourceKind;
    fn connector_version(&self) -> &str;
    async fn discover(&self, subject: &ResolvedSubject, cursor: Option<DiscoveryCursor>)
        -> Result<DiscoveryPage, ConnectorError>;
    async fn fetch(&self, document: &DiscoveredDocument)
        -> Result<FetchedDocument, ConnectorError>;
    async fn healthcheck(&self) -> Result<ConnectorHealth, ConnectorError>;
}
// Normalize / persist dans talaria-api ou talaria-store services — pas dans le connecteur HTTP.
```

HTTP : timeouts, retries expo + jitter **uniquement** erreurs transitoires, `Retry-After`, circuit breaker si présent, User-Agent Talaria, ETag/Last-Modified, pagination bornée, métriques. Pas de retry sur 4xx non transitoires. Respecter le rate-limit Lot E déjà amorcé.

---

## Recherche d’une personne / Ranking / Droits

Mêmes règles métier que le prompt d’origine (`SubjectSearchProfile`, matching explicable, score versionné avec composantes stockées, politiques `metadata_only`, suppression logique d’accès public sans casser l’audit).

---

## API v1 attendue (Axum, préfixe existant `/api/v1`)

Ajouter (pagination curseur, JSON stable) :

- `GET /api/v1/entities/{id}/documents?types=&providers=&academic_status=&access=&language=&cursor=`
- `GET /api/v1/entities/{id}/bibliography?relation=about|by&cursor=`
- `GET /api/v1/entities/{id}/historiography?status=&cursor=`
- `GET /api/v1/documents/{id}`
- `GET /api/v1/documents/{id}/fragments?cursor=` (filtrage droits)
- CLI admin idempotente : ex. `talaria corpus-ingest --subject … --providers theses_fr,hal --limit …` (bornée, observable ; s’intégrer à `source_discovery_runs`)

Exemple bibliographique : conserver le JSON du prompt d’origine (dates typées, identifiers, access.level).

Front : endpoints consommables depuis `web/src/lib/api.ts` ; UI bibliographie/historiographie peut venir en PR séparée.

---

## Idempotence, observabilité, perf

- Réutiliser / enrichir `source_discovery_runs` + métriques JSON.
- Clés : `(source_kind, external_id, revision)` ; `(scheme, normalized_value)` ; fingerprints quality existants.
- Index uniques + FK + curseurs (pas d’offset non borné).
- Secrets via `.env` / env validés au démarrage (`talaria-core::AppConfig`) — documenter les clés (`HAL_*`, `EUROPEANA_API_KEY`, etc.).
- Logs structurés (`tracing`) sans texte intégral sensible ni secrets.

---

## Tests obligatoires

- `cargo test -p talaria-sources` (fixtures only, **no network**)
- `cargo test -p talaria-quality` / store / api selon surface
- Fixtures minimales figées : theses.fr, HAL, Gallica/BnF pour le Lot 1
- Couvrir : normalisation, pagination/reprise, 429/Retry-After, idempotence, ISBN/DOI/ARK/PPN/NNT, non-fusion homonymes, preuve obligatoire, notice ≠ event, droits `metadata_only`, append-only events, pas de N+1 sur requêtes listes

Pas de WebMock/VCR : **httpmock** optionnel en unit tests ou payloads JSON locaux uniquement.

---

## Découpage de livraison

**PR 1 — Socle documentaire vertical**  
Migrations `015+` ; registre étendu ; snapshots/idents/contributions ; interface ; connecteurs **theses_fr + hal** (+ gallica_bnf si tenable) ; CLI + runs ; endpoints documents/bibliographie ; tests + notes d’indexation.

**PR 2 — Corpus et preuves**  
IIIF/OCR/fragments étendus ; droits ; assertion → quality/soft claims ; API citation.

**PR 3 — Historiographie**  
Positions / supports / conflits / revue ; Crossref/OpenAlex/OpenEdition/Persée ; projection Intuition **sans** auto-publish.

**PR 4 — Patrimoine international et médias**  
Europeana, IA/OL ; audio/vidéo timecodé ; OAI-PMH/IIIF génériques whitelist.

Ne pas tout faire dans une seule PR.

---

## Format de sortie exigé

À la fin de chaque session d’implémentation :

1. Diagnostic dépôt + décisions d’architecture  
2. Fichiers créés/modifiés  
3. Migrations, structs Rust, validations, services  
4. Exemples payloads in / réponses API  
5. Tests + commandes + résultats  
6. Indexation, volumétrie, cache, rate limits, sécurité  
7. Limites réelles fournisseurs + variables d’env  
8. TODO actionnables avec critères d’acceptation  
9. **Aucun** commit / push / merge / PR sans demande explicite  

Si trop large : socle + **un** connecteur vertical complet (idéalement **theses.fr** ou **HAL**), tranche migrée, testée, exposée API. Pas de pseudo-code, pas de structs vides, pas de champ fourre-tout.

---

## Documentation officielle à vérifier au moment du développement

- theses.fr / ABES : https://documentation.abes.fr/aidethesespro/co/PrincipeAPI.html  
- HAL : https://api.archives-ouvertes.fr/ et https://api.archives-ouvertes.fr/docs/search  
- BnF/Gallica : https://api.bnf.fr/fr/api-document-de-gallica et https://api.bnf.fr/fr/recherche  
- Europeana : https://pro.europeana.eu/discover-the-data/apis  
- Internet Archive : https://archive.org/developers/index-apis.html  
- Open Library : https://openlibrary.org/developers/api  
- Crossref : https://www.crossref.org/documentation/retrieve-metadata/rest-api/  
- OpenEdition OAI-PMH : https://www.openedition.org/8883  

Ces URL sont des points d’entrée, pas des contrats recopiés. Vérifie paramètres, versions, licences, auth et limites **actuelles** avant chaque connecteur.

---

## Rappels outillage local

```bash
docker compose up -d
cp -n .env.example .env   # DATABASE_URL :5433, TALARIA_DATA_ROOT
cargo run -p talaria-api -- migrate
cargo test -p talaria-sources
cargo run -p talaria-api -- source-status
cargo run -p talaria-api -- serve
```

Après toute nouvelle migration sous `migrations/` : **rebuild** (`cargo build -p talaria-api`) avant `migrate` (sqlx embed).
