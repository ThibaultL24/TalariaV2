# V3.12 — Activation progressive d'Istoria en développement

Date : 18 novembre 2026. Branche `feat/v312-development-activation`, issue de `feat/v311e-activation-prep` (`4475c01`).

La migration `055_editorial_assertions.sql` est appliquée sur `talaria_engine_development`. Aucune ingestion n'a été lancée. Marceline Desbordes-Valmore reste absente. Les 2 218 événements actifs sont inchangés. Aucun chapitre n'est publié. Rien n'est déployé, et `main` n'a pas bougé.

## 1. Sauvegarde et vérification

Avant la migration, la base contenait les versions 1 à 54, sans trou : 54 lignes dans `_sqlx_migrations`, première version 1, dernière version 54. Le seul fichier de migration absent de cette base était `055_editorial_assertions.sql`.

Sauvegarde :

- Fichier : `backups/talaria_engine_development-2026-11-18-pre055.dump`
- Format : `pg_dump -Fc`
- Taille : 140 Mio
- SHA-256 : `19a64e30216ab92e591b191d371377a5a4e8017194eb3d0dc8194db2016f038c`

Cette sauvegarde a été restaurée dans `talaria_v312_restore_check`. La base restaurée portait la migration 54, 3 517 événements canoniques dont 2 218 actifs, 2 218 identités logiques et 47 112 preuves. Ces comptes sont ceux de la base de développement avant l'application. La base temporaire a ensuite été supprimée.

Hôte et port confirmés avant l'écriture : `localhost:5433`, base `talaria_engine_development`.

## 2. Migration appliquée

Une seule migration a été appliquée, avec `TALARIA_APPLY_MIGRATIONS=1` : version 55, description `editorial assertions`.

Elle crée `editorial_assertions` et `editorial_assertion_passages`, leurs contraintes et leurs index. Une assertion peut référencer un groupe éditorial, une identité logique et, par les passages, une preuve existante. Elle ne réécrit pas `canonical_events`.

Les deux tables sont présentes et vides : 0 assertion, 0 passage.

## 3. Contrôles d'intégrité

Relevés après la migration, puis de nouveau après les tests navigateur.

| Mesure | Avant | Après |
| --- | ---: | ---: |
| Migration maximale | 54 | 55 |
| Événements canoniques | 3 517 | 3 517 |
| Événements actifs | 2 218 | 2 218 |
| Identités logiques | 2 218 | 2 218 |
| Preuves | 47 112 | 47 112 |
| Groupes éditoriaux | 0 | 0 |
| Assertions éditoriales | — | 0 |
| Passages d'assertion | — | 0 |
| Tâches d'ingestion | 0 | 0 |
| Entité `Q466987` | 0 | 0 |

## 4. État de l'API

Le serveur de développement écoute sur `127.0.0.1:8081`. Il a été lancé sans `TALARIA_RESEARCH_WRITES`, sans `TALARIA_RESEARCH_QID`, et sans clé de modèle payant.

`GET /health` répond :

```json
{"status":"ok","engine":"talaria","research_writes":false,"research_qid":null}
```

Le worker d'ingestion ne démarre que si les écritures sont ouvertes et qu'une identité est nommée. Les deux conditions manquent, donc aucun worker n'a démarré.

## 5. Tests navigateur

Playwright, Chromium, un worker, projets Desktop Chrome et Pixel 5. Huit tests, huit réussites.

- La recherche « Dumas » distingue Q38337 et Q169150, reste sur `/istoria`, et n'affiche pas « Lancer la recherche ».
- Un nom sans identifiant Wikidata ne crée pas de fiche.
- Marceline Desbordes-Valmore, Q466987, est proposée puis refusée. Le `POST /api/v1/ingest/explorer` répond 403.
- Napoléon, Q517, s'ouvre directement. La frise affiche un canevas Pixi, la carte un canevas MapLibre, et la page des sources le titre « Documents sources ». « Enrichir le corpus » est désactivé.
- Le même parcours Napoléon passe sur le viewport mobile.

Le test unitaire `research_writes_stay_closed_unless_the_process_is_opened` passe. Les trois tests de `identity-offer.test.tsx` passent.

## 6. L'ingestion publique reste fermée

Deux verrous, tous les deux fermés dans le processus en cours :

- `TALARIA_RESEARCH_WRITES` doit valoir exactement `1`. Toute autre valeur, y compris l'absence de variable, refuse le `POST`.
- `TALARIA_RESEARCH_QID` doit nommer un seul identifiant Wikidata. Sans lui, même un processus ouvert n'autorise personne. Avec lui, tout autre identifiant reçoit `research_identity_not_authorized`.

Le bouton React suit `/health`. Il n'apparaît que si les écritures sont ouvertes et si l'identifiant affiché est celui qui est autorisé. Le refus du `POST` reste le verrou réel. Le proxy de développement transmet `/health` vers l'API, pour que l'écran lise l'état du serveur.

## 7. Plan du premier pilote, non lancé

Identité proposée : Marceline Desbordes-Valmore, Q466987. Au moment de ce rapport, aucune entité ne porte cet identifiant. Une biographie déjà présente, dont Napoléon Q517, ne doit pas être réingérée.

Un seul appel, après une autorisation distincte :

`POST /api/v1/ingest/explorer`

```json
{"subject":"Marceline Desbordes-Valmore","qid":"Q466987","live":true,"max_documents":8}
```

Cadre du processus qui exécuterait cet appel :

- `TALARIA_RESEARCH_WRITES=1`
- `TALARIA_RESEARCH_QID=Q466987`
- `OPENAI_API_KEY`, `OPENROUTER_API_KEY` et `LLM_API_KEY` absentes
- budget de géocodage vivant laissé à sa valeur par défaut, 8 recherches
- coût maximal de modèle payant : 0

Le quota visiteur existant reste de 6 recherches par heure. Ce pilote n'en consomme qu'une. La réponse contient l'identifiant de la tâche. Cette ligne de `ingest_jobs` est le journal.

Après la tâche : lire chaque événement admis, sa date typée et son lieu ; comparer les comptes canoniques, les identités logiques et les preuves avec le relevé de la section 3. Aucun chapitre n'est publié par cette tâche. Le seuil de suggestion reste 6 événements d'un même type.

Pour arrêter : redémarrer le serveur sans les deux variables. Le `POST` redevient 403 et le worker ne reprend pas la file.

## 8. Conditions pour autoriser cette unique ingestion

1. Une autorisation écrite nomme Q466987 et cette seule tâche.
2. Immédiatement avant l'appel, `entities.qid = 'Q466987'` vaut encore 0.
3. Les comptes sont encore 3 517 événements, 2 218 actifs, 2 218 identités logiques, 47 112 preuves, 0 tâche d'ingestion.
4. Une nouvelle sauvegarde de la base déjà migrée en 55 est prise et sa restauration est vérifiée. La sauvegarde de la section 1 ramène la base à l'état d'avant 055 ; elle sert à cet incident-là.
5. Le processus a les deux variables, aucune clé de modèle payant, et `max_documents` vaut 8.
6. Le serveur est redémarré sans les deux variables dès que la tâche est terminée ou interrompue.

## 9. Risques encore ouverts

La lecture des acteurs repose encore sur des règles lexicales. Le rejeu de V3.11e a examiné 13 événements d'un seul corpus. Il ne mesure pas une vie philosophique ancienne, ni l'ensemble des biographies déjà en base.

La date absente reste absente. Un lieu n'est cartographié que s'il est le lieu unique et justifié de l'action. Douai, Lyon et Paris l'étaient dans le rejeu ; un pays ou deux villes dans la même proposition ne le sont pas.

La suggestion de chapitre reste vide tant qu'un type n'atteint pas six événements. Ce seuil n'a pas été abaissé.

La recherche Wikidata utilisée par l'écran est une lecture. Elle ne crée pas d'événement. L'ouverture ultérieure à des personnalités variées, dont une comparaison avec Aristote, attend le résultat revu de ce premier pilote.

## Correction d'un fait après insertion

Pour un fait reconnu erroné après coup :

1. Identifier la ligne canonique et son `logical_event_id`.
2. Conserver `event_evidence` et les passages.
3. Si une correction est soutenue par une citation déjà attachée à cette ligne, `supersede_person_event` ajoute le successeur sur la même identité logique et retire la ligne précédente de l'affichage public.
4. Si aucune correction n'est soutenue, désactiver cette seule ligne (`is_active = false`) et garder la trace de la décision. Les autres biographies ne bougent pas.
5. Ne pas relancer l'ingestion du corpus pour corriger un événement.

Une restauration complète reste réservée à une base endommagée. Pour l'état d'avant cette migration, l'artefact est la sauvegarde de la section 1.
