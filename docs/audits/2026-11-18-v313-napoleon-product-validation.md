# V3.13 — Expérimentation historique et utilisateur sur Napoléon

Date : 18 novembre 2026. Branche `feat/v313-napoleon-product-validation`, issue de `feat/v312-development-activation` (`77aad2c`).

La base `talaria_engine_development` n'a reçu aucune écriture. Elle reste en migration 55. Napoléon y compte toujours 465 événements actifs. Les écritures documentaires y sont fermées. L'ingestion de Q517 a eu lieu seulement dans `talaria_v313_ephemeral`. Aucun chapitre n'a été publié. Rien n'a été déployé, et `main` n'a pas bougé.

## 1. Parcours et URL

L'API de développement écoute sur `http://127.0.0.1:8081`. `GET /health` répond `research_writes: false` et `research_qid: null`. L'écran est servi par Vite sur `http://127.0.0.1:5173`, avec le proxy vers cette API. Les deux processus sont laissés ouverts.

Parcours à ouvrir soi-même :

- Recherche : `http://127.0.0.1:5173/istoria`
- Fiche : `http://127.0.0.1:5173/istoria/14eb944a-fa31-459a-be7d-1f32488e2bd6/overview`
- Frise : `http://127.0.0.1:5173/istoria/14eb944a-fa31-459a-be7d-1f32488e2bd6/timeline`
- Carte : `http://127.0.0.1:5173/istoria/14eb944a-fa31-459a-be7d-1f32488e2bd6/map`
- Sources : `http://127.0.0.1:5173/istoria/14eb944a-fa31-459a-be7d-1f32488e2bd6/sources`
- Naissance : `http://127.0.0.1:5173/istoria/14eb944a-fa31-459a-be7d-1f32488e2bd6/timeline?event=9186bea5-abb7-4c67-b9c6-dfbbf230bc34`

Le parcours effectué :

1. La page Istoria s'ouvre en français, avec la recherche et les trois univers.
2. « Napoleon » propose d'abord Napoléon Bonaparte, Q517, connu localement. La liste ajoute ensuite des intitulés qui ne sont pas des personnes : la retraite de Moscou, l'armée, un plan d'attaque à Ligny. Le panneau est coupé par l'image du bandeau.
3. Le choix de Q517 ouvre la fiche. Le bouton « Enrichir le corpus » est désactivé. Le rappel affiché dit qu'une recherche terminée n'est pas une vérification.
4. L'aperçu donne 261 événements cartographiés, 189 lieux et 465 moments.
5. La frise Pixi est présente. La période affichée au premier écran est 1769–1808. Les filtres comptent 200 moments, 95 anecdotes, 57 batailles, 22 charges, 13 voyages, 7 faits familiaux et 6 œuvres. Le texte dit que 465 moments sont sur la frise. Des boutons proposent toute la période, les tiers, et « Charger d'autres moments ».
6. La naissance s'ouvre depuis la frise : `born_in`, 1769, Ajaccio. Le passage dit « naît à Ajaccio le 15 août 1769 ». Le jour est dans la phrase et absent de la date enregistrée.
7. La carte MapLibre est présente, avec une légende par genre et deux champs d'années. Elle commence sous le bloc des suggestions. Dans la fenêtre par défaut, une grande partie du canevas reste hors de la zone visible, parce que le conteneur immersif coupe le débordement.
8. Les sources affichent 147 documents. Le compteur Wikipedia de cette page vaut 0. OpenAlex 38, HAL 29, theses.fr 25, Internet Archive 32, Persée 16, Gallica 7. La page dit qu'une notice de catalogue n'est pas la preuve d'un événement.
9. Sur un écran étroit, l'en-tête devient « Retour », le nom, et « Recherche et filtres ». Les onglets restent atteignables. Les suggestions occupent encore l'écran avant la frise.

## 2. Corpus déjà présent

Entité `14eb944a-fa31-459a-be7d-1f32488e2bd6`, Q517, pipeline `person`. Lecture seule.

| Mesure | Valeur |
| --- | ---: |
| Lignes canoniques | 677 |
| Actives | 465 |
| Inactives | 212 |
| Identités logiques actives | 465 |
| Jour exact | 2 |
| Année exacte | 455 |
| Année approximative | 8 |
| Intervalle | 0 |
| Sans date | 0 |
| Cartographiés | 261 |
| Libellés de lieu | 189 |
| Lieux Wikidata distincts | 106 |
| Preuves | 6 163 |
| Événements sans preuve | 0 |

Les deux seuls jours exacts sont Mondovì, 21 avril 1796, et Lodi, 10 mai 1796. Le siège de Mantoue n'est pas une ligne à part : aucun intervalle n'est stocké pour cette personne.

Répartition active : bataille 190, fait historique 168, charge 17, arrivée 15, diplomatie 15, résidence 10, mariage 9, rencontre 8, publication 6, départ 5, formation 5, exil 4, voyage 4, siège 2, puis une ligne pour la naissance, la mort, l'inhumation, la santé, un procès et quelques types isolés.

Décennies des dates projetées : 1760 : 5, 1770 : 5, 1780 : 15, 1790 : 79, 1800 : 165, 1810 : 179, 1820 : 14, puis 1850, 1869 et 1870 : une ligne chacune.

Preuves par type de document : OpenAlex 3 280 passages pour 249 événements, HAL 1 335 pour 216, theses.fr 684 pour 176, Wikipedia 474 pour 452, Persée 216 pour 60, Gallica 153 pour 71, Wikidata 21 pour 15. Chaque événement actif a au moins une preuve. 147 événements n'ont qu'une preuve Wikipedia. 305 en ont une et aussi une notice de catalogue. Un échantillon OpenAlex est un titre bibliographique, « The Eighteenth Brumaire of Louis Bonaparte », avec un DOI. Ce titre nomme un livre de Marx. Il ne raconte pas le 18 brumaire.

Treize lieux affichés sont des identifiants `Q…`. Treize points cartographiés tombent sur 47° N, 2° E, le centre géométrique de la France. Le sacre du 2 décembre 1804 est l'un d'eux : le libellé est « France », pas Notre-Dame.

## 3. Référentiel de couverture

Ce tableau sert à voir ce que le moteur retrouve. Il n'est pas une liste à insérer.

| Repère | Repère établi | Dans le corpus actuel | Dans la base éphémère |
| --- | --- | --- | --- |
| Naissance, 15 août 1769, Ajaccio | Établi | Année 1769, Ajaccio. Le jour est dans le passage. | Jour 15 août 1769, Ajaccio, cartographié |
| Autun, puis Brienne, vers 1779–1784 | Établi | Mentions d'Autun et de Brienne, parfois datées 1778, 1779 ou 1784 | Brienne sans date ; École militaire en 1784, sous forme d'identifiant |
| École militaire, Paris, 1784 | Établi | Une formation 1784 dont le lieu est `Q273480` | `studied_at Q273480`, année 1784, sans point |
| Toulon, septembre–décembre 1793 | Établi | Le mot apparaît dans des titres | Absent |
| 13 vendémiaire, 5 octobre 1795 | Établi | Le mot apparaît | Absent comme événement lisible |
| Mariage avec Joséphine, 9 mars 1796 | Établi | Année 1796, lieux Italie, Espagne, et l'identifiant de Joséphine | Année 1796, identifiant de Joséphine, sans lieu |
| Montenotte, 12 avril 1796 | Établi | Une bataille 1796, lieu Montenotte. Le jour est dans le titre, l'année seule est enregistrée. | Absent |
| Mondovì, 21 avril 1796 | Établi | Jour exact, Mondovì | Absent |
| Lodi, 10 mai 1796 | Établi | Jour exact, Lodi | Absent |
| Lonato, 3–4 août 1796 | Établi | Année 1796, lieu enregistré « Italy » | Absent |
| Castiglione, 5 août 1796 | Établi | Aucun titre ni lieu | Absent |
| Arcole, 15–17 novembre 1796 | Établi | Aucun titre ni lieu | Absent |
| Rivoli, 14–15 janvier 1797 | Établi | Une bataille 1797, lieu Rivoli | Absent |
| Siège de Mantoue, 4 juin 1796 – 2 février 1797 | Établi, intervalle | Aucune ligne propre, aucun intervalle | Absent |
| Campo Formio, 17 octobre 1797 | Établi | Aucun titre ni lieu | Une arrivée à Paris le 5 décembre 1797 porte ce titre |
| Égypte et Syrie, 1798–1799 | Établi | Sept lignes dont le lieu est « Egypt » | Malte, 9 juin 1798. Une phrase sur Alexandrie est attachée à la mort. |
| 18 brumaire, 9–10 novembre 1799 | Établi | Le mot apparaît | Une charge « Premier consul » en 1799, sans récit |
| Concordat, 1801 | Établi | Cinq titres contiennent le mot | Absent |
| Code civil, 1804 | Établi | Un titre | Absent |
| Sacre, 2 décembre 1804, Notre-Dame | Établi | Une ligne dont le lieu est Lodi, et le sacre cartographié au centre de la France | Charge « empereur des Français » en 1804, sans lieu |
| Austerlitz, 2 décembre 1805 | Établi | Plusieurs lignes en 1805 et une en 1806 | Absent |
| Russie, 1812 | Établi | Moscou et Borodino apparaissent dans des titres | Absent |
| Leipzig, 16–19 octobre 1813 | Établi | Le mot apparaît dix fois | Absent |
| Elbe, 1814–1815 | Établi | Le mot apparaît | Résidence 1814, identifiant de l'île, sans point |
| Waterloo, 18 juin 1815 | Établi | Huit titres et six lieux Waterloo | Absent |
| Sainte-Hélène, mort le 5 mai 1821 | Établi | Mort en 1821, lieu Saint Helena. L'inhumation aussi. | Mort en 1821, Longwood House, cartographié. Le jour manque. |

Traditions à tenir à l'écart d'un fait public : la formule exacte des pyramides, « pas ce soir Joséphine », la bataille de boules de neige à Brienne lue comme un présage, et l'idée que toucher les pestiférés de Jaffa aurait été un geste miraculeux. La visite aux malades est une scène représentée ; l'immunité ne l'est pas.

## 4. Ingestion isolée

Base `talaria_v313_ephemeral`, migration 55, vide au départ. Serveur séparé sur `127.0.0.1:8082`, avec `TALARIA_RESEARCH_WRITES=1`, `TALARIA_RESEARCH_QID=Q517` et un budget de géocodage de 8. Un seul appel :

`POST /api/v1/ingest/explorer` avec Q517, `live: true`, `max_documents: 8`.

Tâche `0a5e05e1-6a03-4e8e-8152-08c7cac7c835`, terminée. Le rapport de tâche dit 2 pages Wikipedia, 30 faits insérés, 4 renforcements, 87 documents découverts et 2 documents rapprochés. La base contient 30 événements actifs, 34 passages, 8 documents (5 Wikipedia, 1 Wikidata, 1 OpenAlex, 1 HAL) et 0 ligne dans `claims`.

Limite de ce budget : deux pages narratives ne peuvent pas porter Toulon, la campagne d'Italie, le sacre, Austerlitz, la Russie ni Waterloo. Les charges de Wikidata arrivent comme des triplets, sans phrase.

Écart à la consigne : le processus a rechargé la clé OpenAI depuis `.env` après qu'elle ait été retirée de l'environnement. Quatre appels `person_extraction` vers `gpt-5.4` sont enregistrés dans `ai_runs` (latences 48 s, 20 s, 31 s et 25 s ; 120 éléments renvoyés, dont 30 ont passé les portes). Le serveur éphémère a été arrêté ensuite. L'appel n'a pas été relancé. La base de développement n'a pas reçu ces appels.

## 5. Lecture des 30 faits nouveaux

| Fait admis | Lecture |
| --- | --- |
| Naissance, 15 août 1769, Ajaccio | Le passage donne le jour, les parents et la maison. Fait établi, bien placé. |
| Résidence le même jour à Ajaccio | La même phrase dit que les parents y vivaient. Second fait, même phrase. |
| Résidence 1769, identifiant d'Ajaccio | Triplet Wikidata, sans point. Double la naissance. |
| École militaire, 1784 | Triplet. L'année correspond à l'entrée à Paris. Le nom de l'école n'est pas affiché. |
| Brienne, sans date | La phrase parle de ses dernières années à Brienne et de Paoli. Le séjour n'a pas d'intervalle. |
| Bourse pour une académie « en France continentale » | Phrase juste, point placé vers 46,2° N, 2,2° E. C'est un centre de pays, pas une école. |
| Baptême catholique, sans date | La phrase de baptême est juste. Une autre phrase, le jugement d'un examinateur sur son niveau en mathématiques, est attachée au même fait. |
| Service à Valence et Auxonne jusqu'en 1789 | La phrase le dit. Aucun des deux lieux n'est retenu. |
| Charge du 10 août 1792, lieu « Its Defence » | La phrase dit qu'il a vu le massacre des Suisses trois ans plus tôt et que l'artillerie déciderait de la défense. Le 10 août est le souvenir, pas une charge exercée ce jour-là. Le lieu est un morceau de phrase anglaise. |
| Résidence à Paris, 1792 | Triplet `Q90`. Paris n'est pas écrit, et il n'y a pas de point. |
| Mariage, 1796, Joséphine | Triplet. L'année est juste. Le 9 mars et le lieu manquent. |
| Tarvis, mars 1797 | La phrase dit que Bonaparte repousse Charles après Tarvis. Le combat du 21–23 mars 1797 est mené dans son armée. Le point est à Tarvis. L'attribution personnelle reste à relire, comme pour les combats d'Augereau ou de Murat. |
| Retour à Paris, 5 décembre 1797 | La phrase le dit. Paris est le bon point. |
| Titre « Campo Formio », arrivée à Paris le 5 décembre 1797 | La phrase jointe parle de 150 000 prisonniers et de 18 batailles. Elle ne dit ni Campo Formio, ni Paris, ni le 5 décembre. Le traité est du 17 octobre 1797. Fait mal assemblé. |
| Malte, 9 juin 1798 | Une ligne a le jour et pas le lieu. Une autre a le lieu et pas le jour. Les deux viennent du voyage vers l'Égypte. |
| Constantinople, « il n'occupe jamais le poste » | La phrase dit que le transfert est accordé et qu'il ne prend pas le poste. Le point est pourtant à Constantinople. |
| Lisbonne, titre « traité de Bâle (1795) » | La phrase parle du moment où il décide d'intervenir en Espagne, et cite Bâle comme l'alliance de 1795. Elle ne raconte pas une arrivée à Lisbonne. |
| « Traité de Versailles », lieu Corse, sans date | La phrase dit qu'il naît un an après la cession de la Corse. Le fait de contexte est juste. Le titre ressemble à un traité qu'il aurait conclu. |
| Rencontre, février 1802 | La phrase raconte le débarquement de Leclerc à Saint-Domingue. Napoléon envoie l'expédition. Il ne débarque pas. Le type « rencontre » ne convient pas. |
| Premier consul, 1799 | Triplet, sans récit du 18 brumaire. L'année est la bonne. |
| Président de l'Académie des sciences, 1800 | Triplet seul. Aucune phrase de ce lot ne le raconte. |
| Empereur des Français, 1804 et de nouveau 1815 | Deux triplets. Ils correspondent au Consulat impérial puis aux Cent-Jours. Aucun ne dit le 2 décembre ni le 20 mars. |
| Monarque d'Italie, 1805 | Triplet. L'année du royaume d'Italie est la bonne. Le couronnement de Milan n'est pas daté. |
| Co-prince d'Andorre, 1806 | Triplet seul. |
| Mariage, 1810, Marie-Louise | Triplet. L'année est juste. Le 1er–2 avril manque. |
| Elbe, 1814 | Triplet, sans point et sans le nom de l'île à l'écran. |
| Sainte-Hélène, 1815 | Triplet de résidence, sans point. |
| Mort, 1821, Longwood House | Une phrase dit le cancer en 1821. Le point de Longwood est justifié. Le 5 mai manque. Une phrase sur le débarquement à Alexandrie et Chébreiss est attachée à cette mort. |

## 6. Comparaison

Le corpus actuel est abondant et grossier. Presque tout est daté à l'année. Il couvre le vocabulaire d'Austerlitz, de Waterloo, de Leipzig et de la Russie, souvent par plusieurs lignes pour le même nom. Il cartographie 261 lieux, dont le centre de la France.

Le corpus neuf est court et inégal. Il retrouve le jour de naissance, Ajaccio, le retour à Paris du 5 décembre 1797, Malte au 9 juin 1798, et Longwood. Il retrouve aussi des charges réelles sous forme d'identifiants. Il publie en même temps Campo Formio à Paris, Bâle à Lisbonne, une défense qui n'est pas un lieu, un poste jamais occupé et une expédition de Leclerc classée comme rencontre.

Les faits communs, au sens d'un même épisode reconnaissable, sont la naissance à Ajaccio, un séjour scolaire, le mariage avec Joséphine, le mariage avec Marie-Louise, une présence à Paris, l'île d'Elbe, Sainte-Hélène et la mort. Le corpus neuf date mieux la naissance. Le corpus actuel la laisse à l'année alors que son propre passage contient le 15 août.

Le corpus neuf ne retrouve pas Toulon, Vendémiaire, Mantoue, Arcole, Castiglione, Rivoli, le sacre comme cérémonie, Austerlitz, la Russie, Leipzig ni Waterloo. Ce manque vient du budget de deux pages effectivement lues, pas d'une supériorité du corpus long.

## 7. Campagne d'Italie

Dans la base de développement, les lignes propres sont Montenotte (1796, lieu Montenotte), Mondovì (21 avril 1796), Lodi (10 mai 1796), Lonato (1796, lieu « Italy ») et Rivoli (1797). Arcole, Castiglione, Dego, Cherasco, Mantoue et Campo Formio n'ont ni titre ni lieu propres. Une messe du sacre est localisée à Lodi : le mot a suffi.

Les conclusions des lots précédents restent les mêmes, et elles n'ont pas été écrites ici. Mantoue, si elle entre un jour, garde l'intervalle du 4 juin 1796 au 2 février 1797. Ceva, porté par Augereau, et Tolentino, porté par Murat, restent en revue. Tarvis, dans la base éphémère, est du même ordre : la phrase met la victoire au compte de Bonaparte, alors que le combat est celui de son armée.

## 8. Chapitres

Aucun chapitre n'est publié. Les suggestions actuelles regroupent les types dès que six événements du même genre existent. Pour Napoléon, l'écran propose :

- Œuvres, 6
- Campagnes, 192
- Relations, 17
- Engagements politiques, 32
- Résidences, 10

Formation et exil restent sous le seuil. « Campagnes » avec 192 batailles n'est pas un chapitre lisible. Le seuil de six évite les chapitres vides. Il ne sépare pas l'Italie de la Russie, ni une charge d'un lieu.

Méthode universelle à étudier ensuite, sans règle au nom de Napoléon : un chapitre suggéré serait un ensemble daté, borné en nombre, dont les événements partagent une période et soit un lieu dominant, soit un rôle dominant. Le récit affiché viendrait des passages de ces événements. Une suggestion de 192 lignes serait découpée. Le seuil de six resterait le plancher, plus un plafond. Rien de cela n'est codé dans ce lot.

La base éphémère ne propose pas ces chapitres : ses comptes par type restent sous six, sauf si l'on additionnait des charges encore écrites en identifiants.

## 9. Carte et frise

La carte distingue déjà batailles, voyages, charges, études et anecdotes, et elle accepte un intervalle d'années. Elle ne trace pas d'itinéraire. C'est le bon choix : une suite de points ne prouve pas le chemin entre eux. Les défauts sont ailleurs. Le centre de la France reçoit 13 points, dont le sacre. Lonato est placé sur « Italy ». Treize libellés sont des identifiants. La carte est physiquement sous les suggestions et coupée par la mise en page.

La frise montre des années, des filtres et le passage du fait choisi. Le premier cadre s'arrête en 1808, alors que 179 événements actifs tombent dans les années 1810 et que la mort est en 1821. Le lecteur doit comprendre qu'il voit une fenêtre, puis demander le reste. Le jour présent dans une phrase, comme le 15 août 1769, n'est pas la date du point.

## 10. Limites et priorités

Limites documentaires : les notices OpenAlex, HAL et theses.fr multiplient les passages sans raconter l'événement. Wikipedia porte presque toutes les lignes, souvent à l'année. Le budget de huit documents, dont deux pages réellement exploitées, ne reconstruit pas une vie impériale.

Priorités d'interface :

1. Montrer la frise et la carte avant les suggestions.
2. Ouvrir la frise sur l'intervalle qui va de la naissance à la mort, ou indiquer que 1809–1821 est hors du cadre.
3. Réserver la recherche aux personnes et laisser la liste entièrement visible.
4. Écrire les noms à la place des identifiants `Q…`.
5. Garder le passage qui prouve le fait sur la fiche, et laisser les notices de catalogue dans une liste séparée.
6. Reprendre le jour quand la phrase attachée le donne.

Priorité historique, avant tout enrichissement de la base de développement : une phrase ne doit devenir un titre que si elle énonce ce titre ; un pays, une région, ou un poste jamais occupé ne devient pas un point ; un débarquement de Leclerc ne devient pas une rencontre de Napoléon.

## 11. Recommandation

Je ne recommande pas d'enrichir maintenant `talaria_engine_development` avec une nouvelle ingestion de Napoléon. Le corpus actuel se consulte. Le moteur neuf, sur deux pages et malgré quatre appels de modèle non voulus, retrouve le jour de naissance et quelques ancres, et il publie aussi des assemblages faux.

Le prochain essai, s'il est autorisé à part, devrait repartir d'une base éphémère, sans clé de modèle, avec un budget de pages plus large et une relecture de chaque fait admis. Les corrections de Mantoue, d'Augereau et de Murat restent hors de la base de développement. Le moteur doit continuer à servir n'importe quelle vie : les règles ci-dessus portent sur la phrase, le lieu et la date, pas sur Napoléon.
