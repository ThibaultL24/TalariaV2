# Audit Scholar / Visit

Date de lecture du dépôt : 2026-10-14. Le code existant est la source de vérité.

## Ce qui existe

Talaria a un seul pipeline vivant, `pipeline='person'`. L’explorateur (`/explorer`, `/entities/:id/:view`) est déjà Scholar : recherche de personne, ingestion progressive, timeline, carte MapLibre, preuves, filtres de période et de type. Agora (`/agora`) est un espace d’opinions séparé. Les gates de `talaria-quality` refusent un événement participatif après le décès (`EventAfterSubjectDeath`) et laissent passer un type `commemoration` parce qu’il n’implique pas la présence du sujet (`event_implies_subject_presence`). La vue entité excluait déjà `statue`, `museum`, `memorial`, `street_naming` de la vie. Elle n’excluait pas `commemoration`.

Il n’existe pas de produit Visit. Le connecteur `pop_merimee` (`crates/talaria-sources/src/connectors/pop_merimee.rs`) sait interroger POP/Mérimée pour une identité de lieu et des coordonnées. Il n’écrit pas de fiche Visit, ne fournit pas d’horaires, et n’est pas appelé par l’explorateur. OSM et les agendas culturels ne sont pas branchés. Europeana reste un catalogue optionnel.

## Tableau

| Fonctionnalité | Existante | Partielle | Absente | Fichiers |
|---|---|---|---|---|
| Profil de personnalité | oui | | | `web/src/features/entity/entity-page.tsx`, `routes/entity_views.rs` |
| Timeline biographique | oui | | | `timeline-canvas.tsx`, `person_timeline_projection` |
| Carte historique | oui | | | `entity-map.tsx`, `map-canvas.tsx`, geojson person |
| Recherche de lieux | | oui | | lieux portés par les événements, pas un index touristique |
| Monuments et musées | | | oui | aucun modèle patrimonial avant `033_visit_heritage.sql` |
| Commémorations | | oui | | extracteur `posthumous.rs` ; pas de table Visit avant cette évolution |
| Expositions culturelles | | | oui | |
| Itinéraires touristiques | | | oui | |
| Horaires et accessibilité | | | oui | |
| Navigation Scholar / Visit | | | oui avant cette évolution | `app.tsx`, `navbar.tsx` |

## Données déjà utiles à Visit

Les événements person actifs, géolocalisés, de types vécus (`birth`, `residence`, `death`, `education`, `battle`, `travel`, …) peuvent être projetés en sites. La projection ne crée pas de fait nouveau et ne déduit pas qu’un lieu est ouvert. Les hommages doivent rester hors de `canonical_events` ou, s’ils y sont déjà en `commemoration`, hors de la vue de vie.

## Risques

- Relâcher les gates de durée de vie pour « voir » les hommages polluerait Scholar.
- Inventer des horaires ou des temps de trajet rendrait Visit faux.
- Plusieurs extracteurs classent `visited` en `meeting` ou en `travel`. Visit ne doit pas fusionner ces types sans la relation explicite.

## Choix retenus

- Scholar reste l’explorateur actuel, exposé aussi en `/scholar` et `/scholar/:qid`.
- Visit est un graphe à part : sites, liens, activités, commémorations, itinéraires.
- La première source est la projection des événements vécus déjà géolocalisés.
- Sans moteur de routage, l’ordre est à vol d’oiseau et `travel_minutes` reste nul.
- TODO : connecteurs Mérimée, OSM, agendas, et un moteur de routage configuré. Ils ne sont pas simulés.
