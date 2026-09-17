# Explorer UX Felt-style : carte dominante + drawer latéral

Date : 2026-09-14  
Statut : approuvé (product/design, utilisateur Thibault)  
Portée : documentation UX uniquement — aucune implémentation frontend dans ce PR

## 1. Contexte et objectifs

L'explorer actuel (`explorer-page.tsx`) affiche une carte MapLibre plein écran avec :
- une barre de recherche en haut,
- un badge de progression d'ingest,
- une légende en bas à droite,
- une timeline/waveform en bas au centre (`ExplorerMapTimelineBar`),
- un modal centré pour le détail d'événement (`EventDetailCard`).

Cette architecture présente plusieurs limitations UX :
- **Pas de liste scrollable** : l'utilisateur ne voit que les pins sur la carte, sans vue d'ensemble textuelle des événements.
- **Filtres non câblés** : `ExplorerEventFilters` existe mais n'est pas intégré à la page explorer.
- **Sélection modale intrusive** : le détail occupe un modal central qui masque la carte.
- **Pas de synchronisation liste ↔ carte** : impossible de parcourir les événements textuellement tout en voyant leur position géographique.

### Objectif

Adopter un modèle **Felt-style** (https://felt.com) où :
1. La **carte reste dominante** (full-screen, immersive).
2. Un **drawer latéral droit** (ouvrable/fermable) contient : compteurs, filtres, liste d'événements, détail sélectionné.
3. La **timeline existante** (waveform + scrub année) reste en bas, synchronisée avec le drawer.
4. L'UX s'adapte à la **densité** : sparse (Baudelaire) vs dense (Trump/Napoléon).

### Références

- Audit UX frontend existant : `docs/FRONTEND_SEARCH_MAP_UX_AUDIT.md` (PR #32, si présent)
- Progressive ingest : `docs/PROGRESSIVE_UX.md`, PR #19
- Recherche MapLibre : clustering, list↔map sync, time filter via properties

---

## 2. Non-objectifs / hors périmètre V1

| Élément | Raison |
|---------|--------|
| Heatmap | Complexité + cas d'usage non validé pour biographies |
| StoryMap guided mode | Feature P2 — nécessite narration éditoriale |
| Redesign du basemap (tuiles custom) | Effort disproportionné ; `ANTIQUE_MAP_STYLE` et `openfreemap/dark` suffisent |
| Mobile-first responsive | P1.5 — desktop d'abord, adaptation tablet/mobile ensuite |
| Recherche full-text dans le drawer | P2 — filtres catégorie/période suffisent pour V1 |
| Export PDF/image | Hors scope explorer |

---

## 3. Layout UX (modèle Felt)

```
┌─────────────────────────────────────────────────────────────────────────┐
│  Navbar (search box au centre)                                           │
├────────────────────────────────────────────────────────┬────────────────┤
│                                                        │   DRAWER       │
│                                                        │  ┌──────────┐  │
│                                                        │  │ N faits  │  │
│                 CARTE MAPLIBRE                         │  │ M pins   │  │
│                 (full-screen)                          │  ├──────────┤  │
│                                                        │  │ Filters  │  │
│                                                        │  ├──────────┤  │
│           [pins / clusters]                            │  │ Event    │  │
│                                                        │  │ List     │  │
│                                                        │  │ (scroll) │  │
│                                                        │  ├──────────┤  │
│                                                        │  │ Detail   │  │
│                                                        │  │ (if sel) │  │
│                                                        │  └──────────┘  │
├────────────────────────────────────────────────────────┴────────────────┤
│  Timeline Bar (waveform + year scrub)                                    │
└─────────────────────────────────────────────────────────────────────────┘
```

### 3.1 Drawer latéral droit

- **Largeur** : `min(24rem, 40vw)` — assez large pour les titres d'événements, pas trop pour ne pas masquer la carte.
- **État initial** : 
  - Sparse (< 25 événements) → drawer **ouvert** par défaut.
  - Dense (≥ 25 événements) → drawer **fermé** par défaut, toggle visible.
- **Toggle** : bouton discret en haut à gauche du drawer (icône chevron ou liste).
- **Fermeture** : clic sur le toggle ou swipe (mobile P1.5).

### 3.2 Contenu du drawer (de haut en bas)

1. **Compteurs** : `{N} faits · {M} pins` (événements timeline vs événements mappables).
2. **Filtres** : réutiliser `ExplorerEventFilters` (catégories, statuts épistémiques, périodes si disponibles).
3. **Liste d'événements** : réutiliser `TimelineList` / `TimelineItem` avec scroll vertical.
4. **Détail sélectionné** : réutiliser `EventDetailCard` inline (pas modal) quand un événement est sélectionné.

### 3.3 Timeline bar (existant, à conserver)

`ExplorerMapTimelineBar` reste en bas au centre :
- Waveform histogramme par année.
- Slider `untilYear` pour filtrer temporellement.
- Compteurs `{visible}/{total}` événements.

La timeline se synchronise avec le drawer : changer `untilYear` filtre la liste visible.

---

## 4. Comportement MapLibre

### 4.1 Clustering conditionnel

| Condition | Comportement |
|-----------|--------------|
| ≥ 25 pins dans le viewport | Activer clustering (circle layers + count label) |
| < 25 pins | Désactiver clustering, afficher pins individuels |

**Implémentation** :
- Source GeoJSON avec `cluster: true, clusterMaxZoom: 14, clusterRadius: 50` (valeurs initiales, à affiner).
- Layers : `clusters` (cercles agrégés), `cluster-count` (labels), `unclustered-events` (pins individuels).
- Clic sur cluster → `map.getSource('events').getClusterExpansionZoom(clusterId)` → `flyTo` pour déplier.

### 4.2 Labels humains (jamais de QID)

- Les labels affichés (pin tooltip, drawer list) utilisent `entity_label` / `person` / `place_label`.
- Si un label est de la forme `Q\d+` (QID brut), afficher un fallback ou masquer.
- Le backend est déjà censé résoudre les labels ; le frontend applique un garde-fou.

### 4.3 Incertitude visuelle

| Indicateur | Représentation carte | Représentation drawer |
|------------|---------------------|----------------------|
| `coord_precision_level = 'approximate'` | Pin plus petit OU halo flou | Badge « approx. » sur l'item |
| `date_precision = 'year'` | (pas d'effet carte) | Afficher « ~1805 » ou « vers 1805 » |
| `epistemic_status = 'disputed'` | Couleur différente (orange) | Badge épistémique existant |

Optionnel V1 : cercle d'incertitude (rayon = `uncertainty_radius_m` si disponible).

### 4.4 Sélection et highlight

- **Clic pin** → highlight visuel (outline ou taille augmentée) + ouvrir détail dans drawer.
- **Clic item drawer** → `map.flyTo({ center, zoom: 8 })` + highlight pin correspondant.
- **Un seul événement sélectionné à la fois** ; `selectedEventId` dans le store.
- **Popup minimal** : au survol pin, afficher max 1 ligne (titre tronqué) ou rien du tout — le détail est dans le drawer.

### 4.5 Padding et fitBounds

Quand drawer ouvert, `map.setPadding({ right: drawerWidth + 16 })` pour que `fitBounds` ne cache pas de pins sous le drawer.

---

## 5. Synchronisation Drawer ↔ Timeline

### 5.1 Flux de données

```
untilYear (slider) ──┬──▶ filterTimelineUntilYear(allEvents) ──▶ visibleEvents (drawer list)
                     │
                     └──▶ filterGeoJsonUntilYear(geojson)    ──▶ visibleGeoJson (map pins)
```

### 5.2 Scroll-to-view

Quand l'utilisateur change `untilYear` et qu'un événement nouvellement visible est sélectionné, le drawer scrolle pour le montrer.

### 5.3 Viewport sync (optionnel V1.1)

P1.1 : filtrer la liste drawer aux événements visibles dans le viewport carte actuel (`map.getBounds()`). Nécessite écouter `moveend` et recalculer.

---

## 6. Adaptation sparse vs dense

### 6.1 Définition

| Profil | Critère | Exemples |
|--------|---------|----------|
| Sparse | < 25 événements totaux | Baudelaire, figures mineures |
| Dense | 25–200 événements | Trump, personnages modernes documentés |
| Very dense | > 200 événements | Napoléon, monarques, grandes figures historiques |

### 6.2 Comportement adaptatif

| Aspect | Sparse | Dense / Very dense |
|--------|--------|-------------------|
| Drawer initial | Ouvert | Fermé (toggle visible) |
| Clustering | Désactivé | Activé |
| Filtres | Masqués (tous visibles) | Affichés et utiles |
| fitBounds initial | Zoom serré sur les pins | Zoom plus large, padding généreux |
| Timeline waveform | Peut paraître vide — OK | Histogramme significatif |

### 6.3 Seuils configurables

```typescript
const SPARSE_THRESHOLD = 25;
const CLUSTER_THRESHOLD = 25; // peut être différent du sparse threshold
```

---

## 7. Interaction avec progressive ingest

### 7.1 États de chargement

1. **Recherche en cours** : spinner dans search box.
2. **Ingest actif** (`ingestBusy = true`) : `IngestProgressBadge` affiché, données incomplètes.
3. **Ingest terminé** : badge disparaît, données complètes.

### 7.2 Mise à jour incrémentale

- Le polling existant (`INGEST_POLL_MS = 1500`) rafraîchit `allEvents` et `geojson`.
- Le drawer et la carte se mettent à jour sans perdre la sélection ni le scroll position.
- `fitBounds` ne se re-trigger PAS pendant l'ingest (préserver la position utilisateur) — cf. `initialFitDone` ref.

### 7.3 Indicateurs de progression dans le drawer

Pendant l'ingest, le header du drawer peut afficher :
```
{N} faits · {M} pins · ⏳ enrichissement en cours…
```

---

## 8. Cartographie des composants réutilisables

### 8.1 Composants existants à réutiliser

| Composant | Chemin | Usage dans le nouveau design |
|-----------|--------|------------------------------|
| `ExplorerEventFilters` | `web/src/components/filters/explorer-event-filters.tsx` | Drawer → section filtres |
| `TimelineList` | `web/src/components/timeline/timeline-list.tsx` | Drawer → liste scrollable |
| `TimelineItem` | `web/src/components/timeline/timeline-item.tsx` | Items de la liste |
| `EventDetailCard` | `web/src/components/detail/event-detail-card.tsx` | Drawer → détail inline |
| `SourceRefsList` | `web/src/components/detail/source-refs-list.tsx` | Dans EventDetailCard |
| `EventImageHero` | `web/src/components/detail/event-image-hero.tsx` | Dans EventDetailCard |
| `HowItHappened` | `web/src/components/detail/how-it-happened.tsx` | Dans EventDetailCard |
| `MapCanvas` | `web/src/components/map/map-canvas.tsx` | Inchangé |
| `MapSourceManager` | `web/src/components/map/map-source-manager.tsx` | À adapter pour clustering conditionnel |
| `MapLayers` | `web/src/components/map/map-layers.tsx` | À adapter pour layers cluster |
| `MapInteractions` | `web/src/components/map/map-interactions.tsx` | À adapter pour cluster click |
| `MapLegend` | `web/src/components/map/map-legend.tsx` | Position à ajuster (éviter superposition drawer) |
| `ExplorerMapTimelineBar` | `web/src/components/map/explorer-map-timeline-bar.tsx` | Inchangé |
| `IngestProgressBadge` | `web/src/components/explorer/ingest-progress-badge.tsx` | Position à ajuster |

### 8.2 Nouveau composant à créer

| Composant | Responsabilité |
|-----------|---------------|
| `ExplorerDrawer` | Container drawer : toggle, compteurs, filtres, liste, détail |

### 8.3 Hooks et stores existants

| Module | Chemin | Usage |
|--------|--------|-------|
| `useExplorerStore` | `web/src/stores/explorer-store.ts` | `selectedEventId`, `setSelectedEventId`, `hoveredEventId` |
| `usePersonPicker` | `web/src/hooks/use-person-picker.ts` | Recherche, ingest, compteurs |
| `useI18n` | `web/src/lib/i18n.ts` | Traductions |

### 8.4 Utilitaires geo existants

| Fonction | Fichier | Usage |
|----------|---------|-------|
| `filterTimelineUntilYear` | `web/src/lib/geo.ts` | Filtrage timeline par année |
| `filterGeoJsonUntilYear` | `web/src/lib/geo.ts` | Filtrage geojson par année |
| `boundsOfMapFeatures` | `web/src/lib/geo.ts` | Calcul bounding box |
| `spreadStackedMapPoints` | `web/src/lib/geo.ts` | Dé-superposition pins |
| `buildYearHistogram` | `web/src/lib/geo.ts` | Histogramme waveform |

---

## 9. Critères d'acceptation / scénarios de test

### 9.1 Scénario : Baudelaire (sparse, ~10 événements)

| Étape | Comportement attendu |
|-------|---------------------|
| Recherche « Baudelaire » | Suggestions apparaissent, sélection déclenche ingest |
| Ingest terminé | Drawer ouvert par défaut, ~10 items listés |
| Carte | Pas de clustering, pins individuels visibles |
| Clic pin Paris | Pin highlight, drawer scrolle vers l'événement, détail affiché |
| Clic item drawer | Carte flyTo vers le lieu, pin highlight |
| Timeline slider | Filtrer avant 1850 → moins d'événements dans liste et carte |

### 9.2 Scénario : Trump (dense, ~80 événements)

| Étape | Comportement attendu |
|-------|---------------------|
| Recherche « Donald Trump » | Ingest progressif visible |
| Ingest terminé | Drawer fermé par défaut, toggle visible |
| Ouvrir drawer | Liste scrollable, filtres affichés |
| Carte USA | Clustering actif (cercles avec compteurs) |
| Clic cluster NYC | Zoom in, cluster se décompose |
| Filtrer par « office » | Liste et carte ne montrent que les événements de type office |
| Timeline slider 2000–2020 | Filtrage temporel appliqué |

### 9.3 Scénario : Napoléon (very dense, ~300+ événements)

| Étape | Comportement attendu |
|-------|---------------------|
| Recherche « Napoléon Bonaparte » | Ingest long, badge progression |
| Carte Europe | Clustering agressif, plusieurs clusters |
| Zoom Italie | Clusters se subdivisent, pins bataille apparaissent |
| Drawer filtres | Filtrer « battle » → carte ne montre que batailles |
| Sélection Austerlitz | Détail inline dans drawer, sources listées |
| Incertitude date | Événements « vers 1805 » affichent badge approx. |

### 9.4 Invariants à vérifier

- [ ] Aucun QID brut (`Q\d+`) visible dans l'UI.
- [ ] Drawer ne dépasse pas la hauteur écran (scroll interne).
- [ ] `fitBounds` tient compte du padding drawer quand ouvert.
- [ ] Sélection persiste pendant ingest progressif.
- [ ] Fermer drawer ne désélectionne pas l'événement.
- [ ] Timeline et drawer toujours synchronisés sur `untilYear`.
- [ ] Légende ne superpose pas le drawer.
- [ ] Clustering se désactive quand zoom suffisant pour < 25 pins.

---

## 10. Questions ouvertes

| Question | Décision proposée | Statut |
|----------|-------------------|--------|
| Seuil clustering exact (25 vs 30 vs 50) ? | Commencer à 25, ajuster après tests UX | À valider |
| Viewport sync liste ↔ carte (P1 vs P1.1) ? | P1.1 — pas bloquant pour V1 | Différé |
| Animation cluster expand ? | Utiliser `flyTo` natif MapLibre | OK |
| Drawer width responsive ? | `min(24rem, 40vw)` — à tester tablette | À valider |
| Position légende quand drawer ouvert ? | Déplacer en bas à gauche OU masquer | À valider |

---

## Annexe A : Wireframes ASCII détaillés

### A.1 État initial sparse (drawer ouvert)

```
┌──────────────────────────────────────────────────────────────────────────────┐
│ [Logo] ──────────── [ 🔍 Baudelaire_________________ ] ──────────── [Theme] │
├───────────────────────────────────────────────────────────┬──────────────────┤
│                                                           │ ◀ 10 faits·8pins │
│                                                           ├──────────────────┤
│          🗺️  CARTE PLEIN ÉCRAN                            │ ▼ Filters        │
│                                                           │   [Category ▾]   │
│               • Paris                                     │   [Period ▾]     │
│                    • Bruxelles                            ├──────────────────┤
│                                                           │ ┌──────────────┐ │
│                                                           │ │ 1821 · birth │ │
│                                                           │ │ Paris        │ │
│                                                           │ └──────────────┘ │
│                                                           │ ┌──────────────┐ │
│                                                           │ │ 1867 · death │ │
│                                                           │ │ Paris        │ │
│                                                           │ └──────────────┘ │
│                                                           │ ...              │
├───────────────────────────────────────────────────────────┴──────────────────┤
│  ▁▂▃▅▆▇█▇▅▃▂▁  Jusqu'en 1860 ────●─────────────────── 8/10 visibles        │
└──────────────────────────────────────────────────────────────────────────────┘
```

### A.2 État dense avec sélection (drawer ouvert)

```
┌──────────────────────────────────────────────────────────────────────────────┐
│ [Logo] ──────────── [ 🔍 Napoléon Bonaparte_________ ] ──────────── [Theme] │
├───────────────────────────────────────────────────────────┬──────────────────┤
│                                                           │ ◀ 312 faits·287p │
│            ⚫ 45                                          ├──────────────────┤
│                    ⚫ 23                                  │ ▼ Filters        │
│         ⚫ 67                                             │   [battle ✓]     │
│                        • Austerlitz ★                    │   [1800-1815 ✓]  │
│              ⚫ 12                                        ├──────────────────┤
│                                                           │ ┌──────────────┐ │
│                                                           │ │ 1805 battle  │ │
│     Légende                                               │ │ Austerlitz ★ │ │
│     ● battle                                              │ └──────────────┘ │
│     ● office                                              ├──────────────────┤
│     ● travel                                              │ DETAIL           │
│                                                           │ Bataille         │
│                                                           │ d'Austerlitz     │
│                                                           │ 2 déc. 1805      │
│                                                           │ Sources (3)      │
│                                                           │ [Wikipedia] ...  │
├───────────────────────────────────────────────────────────┴──────────────────┤
│  ▁▂▃▅▆▇█▇▅▃▂▁  Jusqu'en 1815 ──────●───────────────── 187/312 visibles     │
└──────────────────────────────────────────────────────────────────────────────┘
```

---

## Annexe B : Références techniques MapLibre

### B.1 Configuration clustering

```javascript
map.addSource('events', {
  type: 'geojson',
  data: geojsonData,
  cluster: true,
  clusterMaxZoom: 14,
  clusterRadius: 50,
  clusterProperties: {
    // Agrégations custom si nécessaires
  }
});
```

### B.2 Layers clustering

```javascript
// Cercles cluster
map.addLayer({
  id: 'clusters',
  type: 'circle',
  source: 'events',
  filter: ['has', 'point_count'],
  paint: {
    'circle-color': [
      'step', ['get', 'point_count'],
      '#51bbd6', 25,
      '#f1f075', 100,
      '#f28cb1'
    ],
    'circle-radius': [
      'step', ['get', 'point_count'],
      20, 25,
      30, 100,
      40
    ]
  }
});

// Labels count
map.addLayer({
  id: 'cluster-count',
  type: 'symbol',
  source: 'events',
  filter: ['has', 'point_count'],
  layout: {
    'text-field': ['get', 'point_count_abbreviated'],
    'text-size': 12
  }
});

// Pins individuels (non clusterisés)
map.addLayer({
  id: 'unclustered-point',
  type: 'circle',
  source: 'events',
  filter: ['!', ['has', 'point_count']],
  paint: {
    'circle-color': '#11b4da',
    'circle-radius': 6
  }
});
```

### B.3 Cluster click handler

```javascript
map.on('click', 'clusters', async (e) => {
  const features = map.queryRenderedFeatures(e.point, { layers: ['clusters'] });
  const clusterId = features[0].properties.cluster_id;
  const source = map.getSource('events') as GeoJSONSource;
  const zoom = await source.getClusterExpansionZoom(clusterId);
  map.easeTo({
    center: (features[0].geometry as Point).coordinates as [number, number],
    zoom
  });
});
```

---

*Fin du document de spécification.*
