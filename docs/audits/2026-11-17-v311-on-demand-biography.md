# V3.11 — recherche universelle et biographie à la demande

Date : 17 novembre 2026. Branche `feat/v311-on-demand-biography`, issue de `feat/v310-controlled-integration` (`5cf2ae1`).

Aucune ligne de `talaria_engine_development` n'a été modifiée. La migration `055` n'a pas été appliquée. Aucun chapitre n'a été publié. Aucun événement canonique existant n'a été réécrit. Une décision technique `Accept` n'est pas une approbation éditoriale.

La démonstration d'une personnalité absente tient sur une base éphémère, avec les phrases et les QID choisis. Le crawl Wikipédia d'Aristote n'a pas été lancé.

## 1. Architecture réutilisée

Le pipeline `person` reste le seul parcours : `resolve → collect → extract → ground → gate → persist`. Rien de ce chemin n'a été réécrit.

| Besoin | Déjà en place | Ajout V3.11 |
|---|---|---|
| Recherche locale puis Wikidata | `GET /api/v1/entities/search`. Moins de cinq réponses locales, et hors mode hors-ligne, Wikidata est interrogé (délai 900 ms). Aucune fiche n'est créée par la recherche. | Décision d'écran : ouvrir, proposer, ou refuser. |
| Homonymes | Les QID distincts restent distincts. Un QID déjà local est marqué connu. | Le choix de l'utilisateur est conservé. Deux noms proches ne fusionnent pas. |
| Identité stable | `require_person_qid` refuse une résolution ambiguë. L'explorateur n'écrit pas d'entité avant le QID. | Istoria ne lance une recherche que si le QID choisi est de la forme `Q` suivi de chiffres. |
| Ingestion durable | Table `ingest_jobs`. La même voie et la même clé (`qid:Q…`) retrouvent le travail déjà en cours. | Au-delà de 8 travaux `queued` ou `running`, une identité nouvelle reçoit HTTP 429 `ingest_capacity`. Le doublon d'un travail déjà admis passe avant ce plafond. |
| Progression | `GET /api/v1/ingest/explorer/{jobId}/status` porte la phase, la page et les comptes. | Un statut `failed` n'est plus traité comme une fin silencieuse. Aucun pourcentage n'est affiché. |
| Qualité | Portes, dates typées, rôles, identités logiques, preuves idempotentes. | `publishes_to_public_corpus` nomme la règle : `Accept` et une attribution qui peut tenir seule. `NeedsReview` et `Reject` restent hors du corpus public. |
| Carte et frise | GeoJSON et Pixi.js lisent les événements canoniques actifs. Un lieu qui n'est pas le locus n'a pas de point. | Les faits admis rejoignent ces lectures. Aucun moteur parallèle. |
| Chapitres | `editorial_groups` : `draft` ou `published`. Le public ne voit que `published`. | Suggestions calculées, jamais écrites dans cette table. |

Pas de nouvelle couche d'agent. Pas de texte narratif produit pour combler un trou.

## 2. Routes

Inchangées dans leur contrat, avec un plafond nouveau :

- `GET /api/v1/entities/search`
- `POST` d'ingestion explorateur, puis `GET /api/v1/ingest/explorer/{jobId}/status`
- timeline et GeoJSON, pipeline `person` par défaut

Modifiées :

- L'admission d'un travail, après la recherche d'un travail déjà actif, compte les travaux actifs. Si `capacity_open` est faux, la réponse est `429` et `{ "error": "ingest_capacity", "active": n }`.
- Le plafond de documents explorateur reste 400 (`resolve_max_documents`).

Nouvelle :

- `GET /api/v1/entities/{entity_id}/chapter-suggestions`
- Compte les événements `person` actifs par type.
- Répond `{ "standing": "suggestion", "published": false, "suggestions": [...] }`.
- N'écrit pas `editorial_groups`.

## 3. Écran

Istoria cesse de lancer l'ingestion à la sélection (`startLifeIngest` est faux pour cette expérience). L'explorateur et Poreia gardent leur déclenchement, et seulement si l'élément choisi a un QID.

- Personnalité déjà en base : la fiche s'ouvre. Le bouton « Enrichir le corpus » n'apparaît que si la fiche a un QID, et il ne part pas tout seul.
- Personnalité absente avec un QID : carte d'identité et bouton « Lancer la recherche ».
- Nom sans QID fiable : « Aucun identifiant Wikidata fiable. Aucune fiche n'est créée. » Pas de bouton.
- La ligne de progression reprend la phase, la page courante et le nombre d'événements de frise. Un échec est une alerte, pas une réussite.
- Les suggestions s'affichent à part des onglets de chapitres publiés.

Fichiers : `identity-offer.tsx`, `chapter-suggestions.tsx`, `istoria-page.tsx`, `use-person-picker.ts`, `use-progressive-ingest.ts`, `entity-context.tsx`, `search-suggestions.ts`, `entity-views.ts`.

## 4. Personnalité déjà présente

Napoléon (Q517) est le cas de l'ouverture. La recherche locale le marque connu. Istoria ouvre la fiche, la frise et la carte déjà servies par les API `person`. Les identités logiques et les événements canoniques existants ne sont pas touchés. L'enrichissement est un second geste, le bouton, qui réutilise le travail déjà en cours si le même QID est déjà en file.

Ce parcours n'a pas été rejoué contre `talaria_engine_development` : le serveur refuse de démarrer tant que la migration `055` est en attente, et l'appliquer était interdit.

## 5. Personnalité absente

Aristote (Q868), Victor Hugo (Q535) et Marceline Desbordes-Valmore (Q466987) sont résolus par QID sur une base éphémère. Deux appels pour Q868 donnent le même identifiant. Q535 est une autre fiche. Aucune fiche n'est créée à partir d'une ressemblance de nom.

La phrase « Aristotle was born in Stagira in 384 BC. » devient un fait canonique : `kind` exact, précision année, début `"-384"`, sans mois ni jour inventés. Six publications distinctes, d'Athènes, en 340–335 av. J.-C., restent six faits parce que l'année typée entre dans la clé d'occurrence.

## 6. Résultat du pipeline d'essai

Le test `an_absent_person_keeps_one_identity_and_only_accepted_facts` passe le même `persist_fact_item` que l'ingestion réelle. Il n'appelle pas Wikipédia.

Admis sur la base éphémère :

- Naissance d'Aristote, 384 av. J.-C., Stagire. `map_eligible` égale la présence d'une géométrie. Aucun point n'est inventé si le lieu n'a pas de géométrie.
- Six publications d'Aristote.
- Résidence de Victor Hugo à Guernesey en 1855. Même règle de carte : le point n'existe que si la géométrie existe. Guernesey est dans le gazetteer ; le test vérifie l'égalité, pas une coordonnée écrite à la main.
- Naissance de Marceline Desbordes-Valmore à Douai en 1786.

Les contrôles de rôle géographique déjà en place restent ceux de `event_place_point`. Un rôle `honorific` ou `mention` n'a pas de coordonnées (`place_is_not_the_event_locus`). Un homonyme de lieu sans identité unique reste non localisé.

## 7. Admis, revue, rejet

La promotion publique est `publishes_to_public_corpus` : décision `Accept` et attribution auto-acceptable (`DirectNameMatch`, `AliasMatch`, `TitleSubjectMatch`, ou action militaire suivie). Le succès technique d'un passage dans le pipeline ne publie pas un candidat.

Les pages biographiques de l'essai (`from_followed_page` faux, titre égal au sujet) passent par `TitleSubjectMatch`. Une page suivie militaire sans rôle démontré reste `MentionedNotActing` et en revue, comme en V3.10. Ceva, Tolentino, Dego et Campo Formio ne changent pas de statut.

Les candidats non acceptés ne sont pas lus par la frise ni par la carte.

## 8. Suggestions de chapitres

`suggest_chapters` ne propose un thème que s'il réunit au moins six événements documentés de ce thème. Naissance et mort ne font pas un chapitre. Un thème absent du corpus n'est pas imposé.

Sur l'essai : Aristote reçoit une suggestion `works` / « Œuvres », standing `suggestion`. Le compte de groupes `published` pour cette fiche est 0. Desbordes-Valmore, avec une naissance seule, n'a aucune suggestion. Un corpus de batailles proposerait « Campagnes » et pas un chapitre de vie générique.

L'écran dit que ces lignes ne sont pas des chapitres publiés. Un brouillon et un chapitre publié restent les statuts `draft` et `published` de `editorial_groups`. Cette branche n'en crée aucun.

## 9. Carte

MapLibre et le GeoJSON existants. Un événement sans géométrie reste hors carte. Le rayon d'incertitude et la précision viennent du gazetteer déjà utilisé (Valmy, 500 m, identité exacte). Les lieux seulement mentionnés n'obtiennent pas de point.

La frise Pixi.js n'a pas été recréée. Un intervalle au jour, dont le siège de Mantoue du 4 juin 1796 au 2 février 1797, garde ses deux bornes. `start_time` reste la projection d'ordre.

## 10. Tests

- `cargo test -p talaria-quality --lib` : 139 passés. Dates antiques, intervalles, suggestions.
- `an_absent_person_keeps_one_identity_and_only_accepted_facts` : passé, base éphémère, environ 74 s. Identité unique, naissance antique, œuvres, résidence, naissance peu documentée, aucun chapitre publié.
- `a_full_research_queue_refuses_a_new_identity_only` : le plafond est ouvert à 0 et à 7, fermé à 8.
- Vitest ciblé : 6 fichiers, 29 tests. Décision de recherche, offre d'identité, suggestions, lecture d'un intervalle, frise.
- `npm run build` : réussi. L'avertissement de taille des chunks MapLibre et Pixi est antérieur.

Le clic dans le navigateur sur Istoria n'a pas été fait. `serve` refuse de démarrer sur la base de développement tant que `055` est en attente, et cette migration ne doit pas être appliquée ici.

## 11. Limites

- Le crawl réel d'Aristote, de Desbordes-Valmore ou d'un enrichissement de Napoléon n'a pas tourné. Wikipédia répond souvent 429, et le budget de pages explorateur est de 400.
- Le plafond de 8 travaux concurrents est un compte, pas un quota par source ni un budget de jetons. Les budgets de connecteurs déjà présents (quinze documents par source sur la voie corpus) restent en place.
- Une reprise après échec réutilise les tentatives déjà prévues sur `ingest_jobs` (trois au plus). Cette branche ne réécrit pas ce bail.
- Sans QID, Istoria ne crée rien. L'explorateur peut encore résoudre un nom unique par `require_person_qid`. Un nom ambigu échoue avec `qid_unresolved`.
- Les suggestions sont des comptes de types. Elles ne lisent pas encore les périodes, les œuvres nommées, ni les relations au-delà du type.
- Aucun LLM n'écrit de chapitre. C'est voulu.
- L'usage mobile est couvert par la mise en page existante d'Istoria, pas par un nouveau test de viewport.

## 12. Activer le parcours sur la base de développement

Ce plan n'a pas été exécuté.

1. Laisser la migration `055` de côté tant qu'une décision séparée ne l'autorise pas. Ne pas lancer `talaria migrate` sur `talaria_engine_development`.
2. Relire les diff de `feat/v311-on-demand-biography` avant tout déploiement. Ne pas fusionner vers `main` dans la foulée.
3. Sur un clone ou une base jetable, jouer d'abord la recherche d'Aristote : choisir Q868, lancer la recherche, puis lire les statuts `accepted`, `needs_review` et `rejected`. Ne pas promouvoir un candidat à la main pour « remplir » la carte.
4. Ouvrir Napoléon (Q517) et vérifier qu'aucun travail d'ingestion ne démarre. N'utiliser « Enrichir le corpus » que pour un complément ciblé, jamais comme un réingest global automatique.
5. Chercher Marceline Desbordes-Valmore en français, confirmer Q466987, et accepter une fiche courte sans chapitre si le corpus reste sous six faits d'un même thème.
6. Contrôler ensuite la frise et la carte : dates antiques en années négatives, intervalles conservés, événements sans géométrie absents du GeoJSON.
7. Créer un chapitre public seulement par une action éditoriale explicite sur `editorial_groups`. Une suggestion ne suffit pas.
8. Surveiller le 429 `ingest_capacity` et les 429 Wikipédia. Ne pas relever le plafond pour accélérer une démonstration.
