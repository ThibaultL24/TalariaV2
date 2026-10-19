# Observabilité des recherches documentaires Agora

## Pourquoi

Les jobs PostgreSQL (migration 044) survivent aux redémarrages ; les checkpoints (045) permettent de reprendre un fournisseur interrompu. La migration **046** ajoute un historique opérationnel **par fournisseur et tentative** : durée, nombre d'appels, documents découverts/récupérés/persistés, échecs, timeouts et correspondances bibliographiques. Les valeurs sont typées (entiers, statut enum, catégorie d'erreur), sans logs HTTP en texte libre.

## API

`GET /api/v1/ingest/{job_id}/metrics?limit=50&offset=0`

Réponse typique (valeurs illustratives, non mesurées) :

```json
{
  "job_id": "424a7382-75ce-4e4f-8040-d71f0c6a77ff",
  "lane": "agora",
  "status": "running",
  "attempt_count": 2,
  "created_at": "2026-10-20T10:00:00Z",
  "started_at": "2026-10-20T10:00:01Z",
  "finished_at": null,
  "coverage": {
    "meaning": "entity_document_match_not_historical_verification",
    "documents_persisted_unique": 12,
    "documents_matched_unique": 7,
    "match_rate_percent": 58.33,
    "providers": [{
      "provider": "hal",
      "documents_persisted_unique": 12,
      "documents_matched_unique": 7,
      "match_rate_percent": 58.33
    }]
  },
  "provider_attempts": [{
    "provider": "hal",
    "attempt": 1,
    "status": "rate_limited",
    "error_kind": "rate_limit",
    "discover_calls": 2,
    "fetch_calls": 12,
    "documents_discovered": 15,
    "documents_persisted": 12,
    "documents_skipped": 3,
    "match_operations": 7,
    "fetch_timeouts": 0,
    "connector_errors": 1,
    "elapsed_ms": 2750,
    "started_at": "2026-10-20T10:00:01Z",
    "finished_at": "2026-10-20T10:00:04Z",
    "interrupted": false
  }],
  "pagination": {"limit":50,"offset":0,"total":1,"next_offset":null}
}
```

L'endpoint accepte uniquement `1 <= limit <= 100` et `0 <= offset <= 10000`. Les métriques sont propres au `job_id` ; la sélection de la personnalité s'appuie sur son QID et son identifiant interne. Le endpoint existant `GET /api/v1/ingest/{job_id}` reste compatible. Les jobs Explorer exposent seulement leur état général et aucune métrique documentaire par fournisseur dans cette première version.

## Sémantique des mesures

- `documents_discovered` : documents ayant franchi la sélection de découverte pour être proposés au fetch (pas nécessairement toutes les correspondances du fournisseur).
- `fetch_calls` : tentatives de téléchargement, y compris erreurs.
- `documents_persisted` : opérations réalisées pendant cette tentative ; peut inclure des documents déjà retrouvés lors d'une reprise.
- `documents_persisted_unique` : documents distincts enregistrés pour le job via `ingest_job_documents`.
- `documents_matched_unique` : documents distincts reliés à l'entité via `entity_document_links` (relation `about`, `by`, `mentioned_in`).
- `match_rate_percent` : taux de liaison documentaire, **pas** un score de véracité historique.
- `status=running` avec `interrupted=true` : une tentative interrompue sans clôture, conservée pour audit.
- `error_kind` : classification prédéfinie ; les messages upstream bruts sont volontairement exclus du endpoint.

Les totaux documentaires ne sont jamais obtenus par addition des tentatives : en cas de retry, cela gonflerait les chiffres. Ils proviennent du ledger unique des documents.

## Exploitation / performance

Indices PostgreSQL : PK `(job_id,source_kind,attempt)` sur `ingest_provider_attempt_metrics`, PK sur `ingest_job_documents`, et index existants sur `entity_document_links`. Le nombre de lignes par job est borné par les connecteurs disponibles et au maximum trois tentatives ; l'endpoint impose une limite de 100 lignes.

Tableau de bord futur : évolution par source des p50/p95 de durée, du taux de timeouts / 429, du taux de liaison documentaire et de la fraction des jobs nécessitant un retry. Distinguer volumes récupérés et nombre d'œuvres réellement pertinentes. Ne pas confondre les sources documentaires d'Agora avec les événements canoniques d'Istoria.

## Déploiement / tests

Migrer d'abord jusqu'à `046_ingest_provider_attempt_metrics.sql`, puis déployer le backend. Les lignes des jobs historiques ne sont pas reconstruites rétroactivement : l'endpoint peut retourner des tableaux vides pour les anciens jobs. Tests SQLx : ownership de lease, compteur et pagination, liens non doublés malgré plusieurs relations. Tests API : contrat des compteurs et absence de fuite du message d'erreur upstream. Vérifier les métriques sur de vrais jobs (Napoléon, Victor Hugo) avant de tirer des conclusions sur les performances.
