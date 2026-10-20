# Agora — reprise documentaire par fournisseur (migration 045)

## Objectif

Les données historiques de Talaria restent centrées sur la personnalité, ses événements canoniques et ses sources. Ce lot optimise uniquement le pipeline documentaire **Agora**. Il n'introduit aucune carte historique ni changement des preuves.

### Fonctionnement

1. L'API enregistre la demande dans `ingest_jobs` (migration 044, déjà en place).
2. À chaque tentative, le worker reçoit le même `job_id` et son nouveau `owner_token`.
3. Le pipeline consulte `ingest_provider_checkpoints` et `ingest_job_documents` :
   - fournisseur terminé avec la même version du connecteur : réutilisation des métriques et absence d'appel `discover/fetch` ;
   - fournisseur interrompu : nouvelle découverte, mais sans `fetch` pour les identifiants `(source_kind,external_id)` déjà sauvegardés ;
   - fournisseur non configuré : signalé dans le rapport, sans prétendre qu'une ingestion a réussi.
4. Les documents sont enregistrés dans le ledger **après** snapshot, normalisation et rapprochement à la personnalité. Une interruption entre les écritures et le ledger peut refaire une opération idempotente au prochain passage.
5. Seul le propriétaire d'un lease actif peut inscrire les documents et les checkpoints.
6. Si un fournisseur renvoie une erreur réseau, un timeout, un HTTP 429 ou un curseur immobile, le pipeline renvoie la liste `incomplete_providers` et le job retourne en file : 30 secondes, puis 60 secondes de backoff. Il s'arrête à trois tentatives.
7. Les connecteurs n'ayant pas échoué et dont la collecte est terminée ne sont plus relancés.

### Contrat

Les endpoints `POST /api/v1/ingest/agora` et `GET /api/v1/ingest/{job_id}` restent inchangés. Le rapport de corpus inclut `incomplete_providers` et `resumable_job_id`. Le job peut passer de `running` à `queued` avant d'atteindre `done` ou `failed`.

### Garanties et limites

- Les événements sont toujours append-only ; un document de catalogue ne crée pas de preuve historique à lui seul.
- Les identifiants Wikidata servent toujours de référence d'identité.
- Une reprise peut refaire la découverte du fournisseur interrompu : elle **ne reprend pas au curseur exact**. Cela évite de perdre une page dont l'enregistrement est incomplet.
- Les quotas par fournisseur et le plafond global continuent d'être appliqués. La liste unique de documents enregistrés est reconstruite depuis le ledger avant toute reprise.
- Les erreurs de normalisation/persistance qui ne sont pas des erreurs de connecteur restent des échecs de job et ne sont pas automatiquement masquées.
- Les variations de version des connecteurs déclenchent une nouvelle consultation du fournisseur concerné.
- Un historique de job réussi reste consultable sans supprimer ses checkpoints.

### Indexation et validation

La migration `045_agora_provider_checkpoints.sql` crée deux index clés primaires : `(job_id,source_kind)` et `(job_id,source_kind,external_id)`, plus un index pour consulter les documents enregistrés et un index partiel sur les jobs en backoff.

Les tests SQL couvrent l'écriture idempotente, la séparation par fournisseur, la perte de lease et le renouvellement de version. Les tests du worker couvrent le backoff, et ceux de l'orchestrateur la décision de réutiliser un checkpoint.

En production, mesurer le nombre de `discover/fetch` évités, la durée de la reprise, le taux de HTTP 429, les tentatives et l'évolution de la pertinence documentaire. Garder `TALARIA_INGEST_WORKERS=1` tant que les quotas multi-instances ne sont pas mesurés.
