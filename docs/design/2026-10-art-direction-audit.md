# Audit direction artistique — 15 octobre 2026

Confrontation du retour du directeur artistique avec le code de `web/`. Les chemins cités par le brief existent, sauf `transcriptions.json` (présent à la racine, non relu phrase par phrase : le brief utilisateur est la source des décisions).

## Décisions non retenues telles quelles

- Pas de libellés Standard / Pro.
- Pas de bandeaux géants pour toute la recherche.
- Pas d’images IA de catalogue.
- Pas de fusion Scholar / Visit / Agora.
- Pas d’assistant conversationnel dans ce lot (étude séparée, non rédigée ici).

## Correspondance

### 1. Première impression

1. Remarque : le projet n’est pas compris dans la première minute.
2. Problème : l’accueil parle de carte et d’Agora, pas des trois intentions.
3. Code : `home-page.tsx` ouvrait sur la recherche et une grille « Figures de la démo » ; Scholar / Visit n’étaient pas nommés ensemble.
4. Applicabilité : totale.
5. Décision : trois liens éditoriaux (Scholar, Visit, Agora) au-dessus du roster.
6. Fichiers : `home-page.tsx`, `i18n.ts`, `app.css`.
7. Risque : les trois phrases restent courtes ; elles ne remplacent pas un hub de personnalité.
8. Acceptation : les trois destinations sont des liens visibles, FR et EN.

### 2. Grandes illustrations

1. Remarque : préférer de grands portraits aux petites cartes.
2. Problème : la grille compacte n’incarne pas les figures.
3. Code : cartes texte + boutons Explorer / Agora, sans image.
4. Applicabilité : partielle — réservé à trois figures (Napoléon, Hugo, Curie).
5. Décision : bandeaux avec miniature Wikipédia (REST summary, lazy) ; le reste du roster reste compact.
6. Fichiers : `home-page.tsx`, `demo-roster.ts`, `app.css`.
7. Risque : vignette absente si Wikipédia refuse ; fallback graphique. Pas de crédit affiché sur la vignette d’accueil (la page source reste celle de Wikipédia).
8. Acceptation : trois liens `/scholar/:qid` ; pas de lien imbriqué ; Visit et Agora sont à côté.

### 3. Fiche cliquable et hub

1. Remarque : les actions semblent dispersées.
2. Problème : l’en-tête d’entité était en anglais figé (« TALARIA · EXPLORER », onglets `overview`).
3. Code : `entity-page.tsx` — quatre `NavLink` plus un lien Visit dans la même barre. Agora n’a pas de route `/:qid`.
4. Applicabilité : partielle. Déjà : `/scholar/:qid`, `/visit/:qid`, `/entities/:id/:view`.
5. Décision : onglets Scholar traduits ; Visit et Agora sur une seconde barre. Agora reste `/agora` sans filtre d’URL (le store n’est pas dans l’adresse).
6. Fichiers : `entity-page.tsx`, `i18n.ts`.
7. Risque : Agora depuis la fiche ne pré-sélectionne pas la personne tant qu’une query `qid` n’existe pas côté page Agora.
8. Acceptation : libellés i18n ; deux niveaux de navigation.

### 4. Carte au survol

1. Remarque : un aperçu immédiat au survol.
2. Problème : `map-interactions.tsx` ne change que le curseur ; le détail exige un clic.
3. Code : `mousemove` → `cursor: pointer` ; `click` → `onSelectEvent`.
4. Applicabilité : totale, non implémentée dans ce passage.
5. Décision : popup MapLibre léger sur desktop, bottom sheet sur tactile, panneau documentaire conservé au clic. Pas de LLM au hover.
6. Fichiers prévus : `map-interactions.tsx`, `entity-map.tsx`.
7. Risque : rerenders et clusters.
8. Acceptation : à écrire avec le lot carte.

### 5. Typographie

1. Remarque : Cinzel ralentit les récits.
2. Problème : le chapeau de dossier utilisait `--font-display`.
3. Code : `cited-paragraph.tsx` (variant lead), `how-it-happened.tsx` (titre de section). Cinzel reste le display des noms (`tokens.css`).
4. Applicabilité : totale pour la prose.
5. Décision : prose et titre de section dossier en `--font-body` (Inter). Cinzel reste sur les noms d’accueil.
6. Fichiers : `cited-paragraph.tsx`, `how-it-happened.tsx`.
7. Risque : faible.
8. Acceptation : plus de `font-display` sur le paragraphe cité.

### 6. Mobile Visit / PWA

1. Remarque : une vraie expérience d’application, surtout pour le tourisme.
2. Problème : Visit est une page web responsive, pas une PWA.
3. Code : `visit-page.tsx`, `visit.css` ; pas de `manifest` web app repéré dans cet audit.
4. Applicabilité : partielle. Pas de Capacitor / Flutter.
5. Décision : lot ultérieur après vérification manifest, cibles tactiles et cache. Hors ligne ≠ horaires à jour.
6. Fichiers : à inventorier.
7. Risque : promettre l’installation store.
8. Acceptation : étude avant code.

### 7. Images d’événements

1. Remarque : illustrer les événements.
2. Problème : une image de lieu ne doit pas passer pour une photo de l’événement.
3. Code : `resolve-event-image.ts` + `event-image-candidates.ts` + `EventImageHero` existent ; la relation média n’est pas affichée comme une classe (événement / lieu / portrait).
4. Applicabilité : partielle. Ne pas dupliquer le résolveur. Pas d’IA de masse.
5. Décision : lot médias — qualifier `kind` déjà présent (`EventImageCandidateKind`) dans l’UI, masquer si licence ou relation absente.
6. Fichiers : `event-image-hero.tsx`, `event-detail-card.tsx`.
7. Risque : vignettes Wikipedia sans attribution visible.
8. Acceptation : à écrire avec le lot médias.

### 8. Assistant

Non retenu dans ce passage. Étude séparée exigée par le brief ; non livrée ici.

## Livré dans ce passage

- Accueil : trois expériences + trois bandeaux portraits + roster compact.
- Fiche : navigation Scholar traduite, Visit / Agora séparés.
- Lecture : dossier en police de corps.

## Carte — aperçu (15 octobre, suite)

Desktop : popup MapLibre au survol d’un point non agrégé (`map-interactions.tsx`), alimenté par les propriétés déjà chargées (`map-preview.ts`). Le clic ouvre toujours la fiche. Mobile (`hover: none`) : bottom sheet, puis bouton « fiche complète ». Un cluster (`point_count`) ne produit pas d’aperçu. Échap ferme le popup. Région `aria-live` mise à jour seulement quand l’événement change.

Agora depuis la fiche : le bouton pose la personnalité dans le store puis ouvre `/agora`. Visit utilise le QID quand il est connu.

Portraits d’accueil : lien vers la page Wikipédia (`Image : Wikimedia`). La licence Commons n’est pas dans le résumé REST ; elle n’est donc pas affichée.

## Non livré

- Tooltip carte et équivalent tactile.
- Qualification visuelle des médias d’événement.
- PWA Visit.
- Étude assistant.
- Captures 390 / 768 / 1440 : non réalisées dans ce passage (pas de session navigateur dédiée).
