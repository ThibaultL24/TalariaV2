# Talaria — trois expériences (audit et plan UX)

Date : 2026-10-18. Base : `origin/main` à `6a1e530` (*Design: distinct Istoria, Poreia and Agora experiences*). Branche de travail PR 1 : `feat/ux-pr1-experience-architecture`. `main` et `V3` ne sont pas modifiées. Aucune migration dans ce lot.

La chaîne SQL de `main` va jusqu’à `042_istoria_landmark_mentions.sql`, y compris ses propres `033`–`035` (visite). Les migrations homonymes d’anciennes branches (`cursor/local-aegean-tourism`, travail parallèle) ne doivent pas être rejouées ici.

## 1. Architecture actuelle

Talaria sert trois promesses déjà nommées dans l’interface, mais deux d’entre elles partagent encore un seul contrôleur.

| Expérience | Route | Contrôleur réel | Données |
|---|---|---|---|
| Istoria | `/istoria`, `/istoria/:entityId/:view` | `EntityPage` (`experience="istoria"`) | `GET /api/v1/entities/:id/overview`, `.../timeline`, `.../bibliography`, `GET /api/v1/events/:id` |
| Poreia | `/poreia`, `/poreia/:entityId/:view` | le même `EntityPage` | overview **et** `.../visit/heritage` pour presque toutes les vues, plus `.../visit/sites`. La carte recharge encore heritage, now et sites en GeoJSON dans `EntityVisitMap` |
| Agora | `/agora` seulement | `pages/agora-page.tsx` | `.../claims`, `.../bibliography`, ingest `POST /api/v1/ingest/agora` |

Le routage vit dans `web/src/app.tsx`. `/explorer` renvoie vers `/istoria`. `/entities/:entityId/:view` renvoie vers Istoria ou Poreia selon `?lens=visit`, en conservant les filtres.

La personnalité courante est dans `useExplorerStore` (`entityId`, `entityLabel`, `entityQid`). La barre du haut (`navbar.tsx`) ignore cet identifiant : Istoria, Poreia et Agora pointent vers la racine de chaque univers. Seul `ExperienceNavigation`, sous le hero, conserve l’UUID — et Agora y perd encore la personne (`/agora` nu).

Backend déjà en place, à réutiliser tel quel :

- Overview, timeline, GeoJSON événements : `crates/talaria-api/src/routes.rs` (`entity_views`, `geojson`).
- Preuves : `GET /api/v1/events/:id` et `.../evidence`.
- Sites Wikidata, GeoJSON, itinéraire, destinations : `visit_wikidata`, `poreia_destinations`.
- Patrimoine historique de visite et activités OpenAgenda : `visit/heritage`, `visit/now`.
- Claims, documents, bibliographie, graphe Agora, commentaires, signaux : routes `claims`, `documents`, `bibliography`, `agora_graph`, `comments`, `claim_signals`.

`canonical_events` reste append-only (`009_canonical_append_only.sql`). Le pipeline explorer est `person`. Les claims portent leurs sources. Rien de tout cela ne change dans les cinq lots UI.

React Query est déjà monté (`IntuitionProviders`) mais sert surtout au wallet. Les pages historiques enchaînent des `useEffect` larges.

## 2. Responsabilités des composants

- `EntityPage` : résolution d’URL, recherche, ingest, overview, timeline ou patrimoine, sélection, frise, carte, sources, fiche. Trop de rôles.
- `EntityMap` / `MapCanvas` / `MapInteractions` : carte de vie MapLibre. Le survol annonce surtout qu’un point est cliquable.
- `HistoricalTimeline` : frise Pixi. La période visible est déjà poussée dans `from` / `to`.
- `EntityVisitMap` : carte touristique. Elle fusionne trois GeoJSON à chaque mouvement de caméra.
- `PoreiaHeritageView`, `VisitSitesList`, `VisitHeritageList` : catalogue. Les sites Wikidata et le patrimoine historique restent deux listes composées à l’écran, pas une projection serveur.
- `PoreiaItineraryView` : ville, puis séquence. Le mode de transport existe encore dans le client (`fetchPoreiaItinerary`, défaut marche) alors que l’UI ne le choisit plus.
- `AgoraPage` + `AgoraPanel` : recherche, ingest, thèses (`claim_kind = theory`), autres claims, bibliographie. Onglets internes en plus de la navigation d’univers. La connexion wallet est dans le hero, avant toute action.
- `Navbar` : univers, langue, thème, wallet. `ExperienceNavigation` : second choix d’univers, avec promesse éditoriale.
- `EventDetailCard` : fait canonique et preuves. Distinct des claims.

## 3. Problèmes UX et impact

1. **Un seul écran pour deux métiers.** Istoria et Poreia partagent le hero, les effets et une partie du vocabulaire. L’utilisateur ne sent pas qu’il change de tâche, seulement d’onglet.
2. **Poreia charge l’historique trop tôt.** Ouvrir la carte touristique déclenche overview, la liste patrimoine complète, les sites, puis trois GeoJSON. Le coût retarde la carte et mélange enquête et visite.
3. **La personne se perd dans la barre principale.** Depuis une biographie, le lien « Poreia » du header retombe sur l’entrée vide. Le lien Agora aussi.
4. **Agora n’a pas d’URL de personne.** Impossible de partager « les thèses sur cette personne ». Le retour vers un fait envoie encore vers `/explorer`, qui oublie l’événement.
5. **Deux navigations d’univers, puis une troisième dans Agora.** Header, cartes Istoria/Poreia/Agora, puis onglets du panneau. Sur mobile, cela pousse le contenu sous la ligne de flottaison.
6. **Poreia parle encore catalogue.** Les vues sont justes, mais l’entrée reste une recherche de personnalité plus une liste. La question « quelle ville, quels lieux » n’est pas le premier écran.
7. **Carte historique sans aperçu.** Le survol ne donne pas le titre, la date typée ni le lieu. La frise et la carte se parlent déjà un peu (`event`, `from`, `to`) sans règle unique pour un événement sans coordonnées.
8. **Agora ressemble à un outil de graphe.** Lecture, popularité et preuve ne sont pas assez séparées. Le wallet est demandé avant l’acte qui en a besoin.
9. **Styles en double.** `tokens.css`, `app.css`, `v3.css`, `agora.css` portent chacun des couleurs, des rayons et des focus.

## 4. Composants à réutiliser

Ne pas réécrire : `EntityMap`, `HistoricalTimeline`, `MapCanvas`, `EventDetailCard`, `EntitySearchBox`, `PoreiaHeritageView`, `PoreiaItineraryView`, `VisitNowList`, `EntityVisitMap`, `AgoraPanel`, `ClaimCard`, `ExperienceNavigation`, `ExperienceDemoFigures`, le store explorer, `entity-views.ts`.

Nouveau socle PR 1, sans second moteur de carte ni de frise :

- `components/layout/experience-layout.tsx` — chrome, hero, navigation d’univers.
- `components/layout/experience-subnav.tsx` — une seule rangée de sections.
- `components/entity/entity-header.tsx` — nom, promesse, passage vers l’autre expérience.
- `components/entity/entity-context.tsx` — URL + store, une personnalité partagée.
- `features/istoria/pages/istoria-page.tsx`
- `features/poreia/pages/poreia-page.tsx`
- `features/agora/pages/agora-page.tsx`

## 5. Modifications proposées

**PR 1 — architecture (ce lot).** Trois contrôleurs. Istoria ne charge la frise que pour Chronologie et Carte de vie. Poreia ne charge plus la timeline canonique, ni la liste patrimoine, quand la vue est la carte : la carte garde ses GeoJSON. Overview (nom, bornes) reste en cache React Query, `retry: false`, pour survivre au passage Istoria ↔ Poreia. La barre principale et la navigation d’univers gardent l’UUID. Agora gagne `/agora/:entityId/:view` (`discover`, `theses`, `arguments`, `sources`) et n’affiche plus ses anciens onglets en double. Les anciennes URL restent des redirections.

**PR 2 — Poreia produit.** Hero méditerranéen, ville, lieux mis en avant, fiche (photo sourcée ou illustration générique assumée, relation, QID/PID, statut de visite, horaires seulement s’ils sont vérifiés). Projection de lecture unique côté API si plusieurs origines décrivent le même site externe — jamais une fusion par coordonnées seules. Parcours sans sélecteur marche/vélo/voiture ; durée affichée seulement si le moteur la fournit.

**PR 3 — Istoria.** Overview éditorial à partir des champs déjà sourcés. Aperçu MapLibre sans fetch au survol. Frise : densité, période visible, points superposés, `prefers-reduced-motion`. Sélection carte ↔ frise ↔ `?event=`, sans inventer de coordonnées.

**PR 4 — Agora.** Parcours Découvrir → Comprendre → Comparer → Participer. Lecture sans wallet. Connexion au moment de publier, soutenir ou contester. Pas de badge de confiance opaque. Deep link de thèse si l’identifiant de claim est stable.

**PR 5 — système visuel.** Tokens, images de `web/public/brand/` selon le rôle (illustration / document / preuve), vues 390 / 768 / 1440, clair et sombre. Mesurer LCP, INP, CLS avant d’annoncer un chiffre. Découper le JS des trois expériences. Playwright seulement si les tests Vitest ne couvrent pas un parcours visible.

## 6. Risques de régression

- Les tests de `entity-page.test.tsx` montent encore `/entities/:id/:view` sur le contrôleur Istoria. Le comportement mobile (navbar, tiroir, canvas) doit rester.
- Un `setEntity` intempestif efface filtres et événement sélectionné dans le store. La synchro ne doit le faire que quand l’UUID change.
- Le cache React Query est un singleton. Les queries de page utilisent `retry: false` pour ne pas multiplier les essais ni ralentir Vitest.
- Rediriger `/istoria/:id` vers `.../overview` ne doit pas casser `/entities/...`, qui n’est pas sous ce préfixe.
- Masquer les onglets internes d’Agora quand la section vient de l’URL. Sans section (aucun autre appelant aujourd’hui), le panneau reste autonome.
- Aucun changement de contrat JSON, aucun snapshot de démo, aucun déploiement.

## 7. Découpage des cinq PR

| PR | Branche prévue | Fichiers principaux | Hors scope |
|---|---|---|---|
| 1 | `feat/ux-pr1-experience-architecture` | `app.tsx`, `navbar.tsx`, `experience-navigation.tsx`, `experience-layout.tsx`, `experience-subnav.tsx`, `entity-context.tsx`, `entity-header.tsx`, pages Istoria / Poreia / Agora, `lib/experiences.ts`, tests de routes | cartes, frise, fiches, wallet, tokens |
| 2 | `feat/ux-pr2-poreia` | `features/poreia/**`, éventuellement une projection `visit` Rust + test SQL si la dédup est serveur, OpenAPI | frise Pixi, protocole Intuition |
| 3 | `feat/ux-pr3-istoria` | `map-interactions.tsx`, `historical-timeline.tsx`, overview Istoria, sync `event` | catalogue touristique |
| 4 | `feat/ux-pr4-agora` | `agora-panel.tsx`, `claim-card.tsx`, wallet, routes de claim | refonte Intuition |
| 5 | `feat/ux-pr5-design-system` | `tokens.css`, `app.css`, `v3.css`, `agora.css`, découpage Vite, mesures | nouvelles données |

Chaque PR part de `main` à jour après fusion de la précédente. Pas de force-push. Pas de fusion tant que `npm test` et `npm run build` dans `web/` ne passent.

## 8. Critères de validation

PR 1 est acceptée quand :

- `/istoria/:id/overview|timeline|map|sources`, `/poreia/:id/map|heritage|itinerary|now` et `/agora/:id/discover|theses|arguments|sources` rendent un contrôleur identifiable.
- `/explorer` et `/entities/:id/:view?lens=visit` redirigent sans perdre l’UUID ni `event`.
- Depuis une personne, le header et la navigation d’univers ouvrent l’autre expérience sur le même UUID.
- La carte Poreia ne demande pas `.../timeline` ni la liste `.../visit/heritage` (les GeoJSON de la carte restent, PR 2 les bornera).
- Istoria Chronologie et Carte de vie demandent encore la timeline ; Aperçu et Sources non.
- La navigation interne est une rangée de liens, utilisable au clavier, avec défilement horizontal sous 390 px.
- `cd web && npm test` et `npm run build` passent.
- Aucune migration, aucun changement OpenAPI.

PR 2 à 5 reprennent les critères des sections produit (fiche sans invention, sync carte/frise, lecture Agora sans wallet, mesures de performance réellement exécutées).
