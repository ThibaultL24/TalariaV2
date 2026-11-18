# V3.14 — Intelligence documentaire et expérience biographique

Date : 18 novembre 2026. Branche `feat/v314-historical-intelligence`, issue de `21bbe0e` (`feat/v313-napoleon-product-validation`).

La branche `integration/talaria-v3-latest` n'existe ni en local ni sur `origin`. Le dernier travail vérifié est V3.13, commit `21bbe0e`. Ce lot part de là. Aucune architecture parallèle n'a été ajoutée.

La base `talaria_engine_development` n'a reçu aucune écriture. Elle reste en migration 55 : 3 517 lignes canoniques, 2 218 actives, 465 événements actifs pour Napoléon, 0 tâche d'ingestion. `GET /health` sur `http://127.0.0.1:8081` répond `research_writes: false` et `research_qid: null`. Aucun chapitre n'a été publié. Aucun appel payant n'a été fait pendant ce lot. Rien n'a été fusionné vers `main`, poussé de force, ni déployé.

## 1. Verrou des modèles

Les quatre appels de V3.13 sont dans `talaria_v313_ephemeral`, tâche `0a5e05e1-6a03-4e8e-8152-08c7cac7c835`. La table `ai_runs` n'a pas de colonne de jetons ni de coût.

| Fournisseur | Modèle | Rôle | Latence | Éléments renvoyés | Heure |
| --- | --- | --- | --- | ---: | --- |
| openai | gpt-5.4 | person_extraction | 47 811 ms | 48 | 2026-11-18 19:05:02Z |
| openai | gpt-5.4 | person_extraction | 19 591 ms | 18 | 2026-11-18 19:05:22Z |
| openai | gpt-5.4 | person_extraction | 30 568 ms | 31 | 2026-11-18 19:05:53Z |
| openai | gpt-5.4 | person_extraction | 24 654 ms | 23 | 2026-11-18 19:06:18Z |

120 éléments sont revenus, 30 avaient été admis. Le processus avait rechargé la clé depuis `.env` après qu'elle ait été retirée de l'environnement.

Le verrou est maintenant explicite. Un appel n'est possible que si `TALARIA_LLM_CALLS` vaut exactement `1`. `TALARIA_LLM_DISABLED=1` l'emporte même si une clé est présente. La valeur par défaut est fermée : charger `.env` ne suffit plus. Le test `a_fake_key_does_not_open_a_model_call` pose des clés fictives et vérifie qu'aucun client n'est configuré. Les deux ingestions de ce lot ont tourné avec `llm_configured: false`, `llm_model: disabled` et `ai_runs = 0`.

## 2. Un fait est une seule phrase

Un titre, un lieu et un jour pris dans des phrases différentes du même passage ne forment plus un fait public. La date d'un événement nommé est lue dans la phrase qui porte ce nom, pas dans le bloc entier.

La relecture isolée de Q517, base `talaria_v314_ephemeral`, budget de 8 documents, verrou fermé, a admis 20 faits. Aucun n'est Campo Formio devenu une arrivée à Paris, le traité de Bâle devenu une arrivée à Lisbonne, « Its Defence » comme lieu, un poste jamais occupé, le débarquement de Leclerc, ni une phrase sur Alexandrie attachée à la mort. Ces passages ne figurent pas dans `event_evidence`.

Ce qui est admis :

| Fait | Date retenue | Lieu | Lecture |
| --- | --- | --- | --- |
| Naissance | 15 août 1769 | Ajaccio | Le jour vient de la parenthèse d'ouverture, pas du triplet `born_in 1769` |
| Résidence | 1769 | aucun | Triplet Wikidata sans lieu. Ce n'est pas la phrase des parents, mais ce n'est pas non plus une résidence située |
| Études | 1784 | aucun | Triplet |
| Service | 1789 | aucun | Valence et Auxonne sont dans la phrase, aucun des deux lieux n'est retenu |
| Résidence | 1792 | aucun | Triplet |
| Mariage | 1796 | aucun | Triplet, sans le 9 mars ni Joséphine |
| Tarvis | 1797, année | Tarvis | La phrase dit mars 1797. Le jour du combat n'y est pas |
| Retour à Paris | 5 décembre 1797 | Paris | La phrase le dit |
| Malte | 9 juin 1798 | aucun | Le jour est gardé. Le second fait « arrivée à Malte » tiré d'un autre paragraphe a été écarté |
| Charges | 1799, 1800, 1804, 1805, 1806, 1815 | aucun | Triplets `held office`, sans le nom de la charge |
| Mariage | 1810 | aucun | Triplet |
| Résidences | 1814, 1815 | aucun | Triplets |
| Mort | 5 mai 1821 | Longwood House | Le jour vient de la même parenthèse. La phrase d'Alexandrie n'est pas jointe |
| Publication | 1795 | aucun | *Clisson et Eugénie* |

Lodi, Castiglione, Arcole, Mantoue et Waterloo ne sont pas dans cette passe. Ils n'ont pas été inventés. Le corpus de développement n'a pas été corrigé. La naissance qui y est stockée, `9186bea5-abb7-4c67-b9c6-dfbbf230bc34`, peut devenir le 15 août 1769 parce que son propre passage anglais le dit. La ligne anglaise de Waterloo dont le passage est « fought on Sunday 18 June 1815 » peut devenir ce jour. Les lignes de 1798 et de 1814 qui parlent de Waterloo ne doivent pas emprunter cette date.

## 3. Couverture documentaire

Le goulot n'est pas le nombre de notices. Sur Q517, budget 8 :

| Mesure | V3.13 | V3.14 |
| --- | ---: | ---: |
| Pages Wikipédia lues pour les faits | 2 | 1 |
| Documents découverts | 87 | 87 |
| Documents téléchargés | non distingué | 9 |
| Documents dont un fait est sorti | 2 pages | 1 |
| Notices rapprochées à un fait | 2 | 0 |
| Preuves ajoutées depuis ces notices | — | 0 |
| Faits admis | 30 | 20 |
| Appels de modèle | 4 | 0 |

Les 87 notices OpenAlex, HAL et autres restent des notices. Aucune n'est devenue une preuve. Huit pages suivies ont été tentées ; une page qui ne donne aucun fait ne consomme pas le budget utile, mais elle n'a pas non plus ouvert un épisode. Monter le nombre de documents sans une phrase qui porte le fait n'augmente pas la biographie.

Victor Hugo, Q535, base séparée `talaria_v314_hugo`, même budget, verrou fermé : 102 documents découverts, 8 téléchargés, 2 utilisés, 4 notices rapprochées, 0 preuve tirée de ces notices, 45 faits, 6 datés au jour, 0 appel de modèle. La naissance est le 26 février 1802 à Besançon. La mort est le 22 mai 1885 à Paris. Le moteur n'est donc pas réglé sur la seule vie militaire.

Cette passe Hugo laisse des assemblages encore faux, à traiter par la même règle de phrase et non par une liste de noms :

- « Spare Abolitionist John Brown's Life » est devenu un lieu. La phrase est une lettre de 1859 demandant la grâce de John Brown.
- « love » est devenu le lieu d'une rencontre. La phrase dit qu'il tombe amoureux d'Adèle Foucher.
- « After the Siege of Paris » est devenu le siège lui-même. La phrase dit qu'après le siège, Hugo revit à Guernesey.
- Une phrase sur Victor Schœlcher est devenue une bataille de 1879.
- « Real Colegio de San » est un début de nom d'école, pas un lieu complet.
- Notre-Dame est le sujet du roman de 1831, pas le lieu de l'acte.

## 4. Carte du corpus déjà présent

Lecture seule, 261 points, 13 libellés qui sont encore un identifiant `Q…`. Rien n'a été déplacé.

Coordonnées répétées au moins cinq fois :

| Latitude | Longitude | Points | Lecture proposée, non appliquée |
| ---: | ---: | ---: | --- |
| 48,86 | 2,35 | 16 | Paris, à relire un par un |
| 47,00 | 2,00 | 13 | Centre de la France, pas un lieu d'action |
| 51,34 | 12,37 | 8 | Aire de Leipzig, à distinguer les faits |
| 42,50 | 12,50 | 6 | Centre de l'Italie |
| −15,95 | −5,72 | 6 | Sainte-Hélène, plusieurs faits au même point |
| 50,93 | 11,59 | 6 | À identifier avant tout déplacement |
| 55,75 | 37,62 | 5 | Moscou |

Un point au centre d'un pays n'est pas le lieu d'un fait. Aucun itinéraire n'a été tracé entre deux événements.

## 5. Périodes et catégories

Les catégories restent un index : Œuvres 6, Campagnes 192, Relations 17, Engagements politiques 32, Résidences 10. Ce ne sont pas des chapitres.

Les périodes sont calculées sur les faits datés. Un écart d'au moins huit ans ouvre une autre période. Une année très chargée reste une période, au lieu d'être coupée en paquets de douze faits. Les titres sont l'intervalle d'années et, quand un lieu domine, ce lieu : 1769–1771 · Ajaccio, 1784–1785 · Brienne, puis une période par année dense de 1793 à 1815. 1815 en contient 50. Aucune période ne s'appelle « Campagne d'Italie ». Rien n'est publié.

Les mêmes règles, sur des faits construits et non sur une liste codée en dur, donnent un séjour à Guernesey pour Hugo, un seul séjour parisien pour Marie Curie, et une seule période de −384 à −322 pour Aristote. Une vie continue sans écart n'est pas découpée pour faire baisser un seuil.

## 6. Écran

L'API de lecture est `http://127.0.0.1:8081`. L'écran est `http://127.0.0.1:5173`.

- Recherche : `http://127.0.0.1:5173/istoria`
- Aperçu : `http://127.0.0.1:5173/istoria/14eb944a-fa31-459a-be7d-1f32488e2bd6/overview`
- Frise : `http://127.0.0.1:5173/istoria/14eb944a-fa31-459a-be7d-1f32488e2bd6/timeline`
- Carte : `http://127.0.0.1:5173/istoria/14eb944a-fa31-459a-be7d-1f32488e2bd6/map`
- Sources : `http://127.0.0.1:5173/istoria/14eb944a-fa31-459a-be7d-1f32488e2bd6/sources`

La recherche « Napoleon » ne propose plus que la personne. L'identifiant Q517 n'est plus affiché dans la suggestion. La frise s'ouvre sur 1769–1870 et dit que les moments se chargent progressivement. La carte et la frise ne sont plus précédées de la liste des périodes ; un lien « Périodes et catégories » y mène. Les sources portent la mention « Notice de catalogue ». Les libellés `resided_in` et les identifiants `Q…` sont rendus en langage courant dans la frise. Sur un écran de téléphone, la carte occupe l'onglet, la légende est un bouton, et les filtres sont repliés.

Pixi.js et MapLibre sont inchangés. Les deux serveurs de développement restent ouverts en lecture. Le serveur d'ingestion éphémère a été arrêté.

## 7. Ce qui reste

Le budget de pages suivies ne transforme pas encore une biographie générale en dossiers d'épisodes. Les triplets Wikidata créent des charges et des résidences sans objet ni lieu. Une mention « après le siège » peut encore devenir le siège. Un syntagme long peut encore devenir un lieu, comme la grâce de John Brown. Les 261 points du corpus de développement n'ont pas été corrigés. Les périodes d'une vie très dense restent des années, pas encore des ruptures de statut.
