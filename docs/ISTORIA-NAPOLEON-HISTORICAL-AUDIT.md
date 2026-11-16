# Istoria — audit historique de Napoléon (Q517)

Première revue exhaustive des événements canoniques actifs de Napoléon Bonaparte dans la base de développement. Les détecteurs sont déterministes. Aucune correction n’a été appliquée.

- Sujet : `Q517`, Napoléon Bonaparte
- Événements actifs constatés au lancement : **465**
- Événements examinés : **465**
- Portée : `all_active_person_events`, pages stables de 100, ordre `start_time`, puis `id`
- Détecteur : `historical_critic:v2`, version `place-role:v2`
- Modèle : aucun appel, 0 jeton, 0 $
- Durée de la revue : **918 ms**
- Journal : run `f2e05fa0-5140-4e01-bc11-18fcea9b19aa`, une tâche, `writes = false`. La seconde écriture du même résultat est restée idempotente.
- Empreinte de contenu avant et après : `be34cc776db35c59754456aba578c983`

L’absence de signal n’est pas une certification historique. Une couverture `bounded` signifie seulement que les passages chargés pour cet événement ont été lus, et qu’une identité géographique fiable existait lorsque le lieu était renseigné. Elle ne signifie pas que toutes les sources possibles ont été consultées.

## 1. Résumé exécutif

La base active de Napoléon est riche et très inégalement précise. Sur 465 événements, 285 ne produisent aucun signal, 89 portent au moins une proposition, 91 demandent une relecture. Il y a 91 propositions pour 203 constats : un événement peut cumuler plusieurs anomalies, et les totaux d’événements ne se confondent pas avec les totaux de constats.

Le fait dominant n’est pas un taux d’erreur de 39 %. C’est la forme des données :

- **457 événements sur 465** n’ont qu’une précision à l’année. Huit dates sont approximatives et inférées. Aucune date stockée n’est au jour ou au mois, alors que des phrases portent déjà le jour.
- **67 propositions** demandent d’enrichir une année déjà juste avec le jour présent dans la même phrase. Ce n’est pas une correction d’année.
- **17 propositions** changeraient l’année. Sept d’entre elles lisent un nombre qui n’est pas une année : `2,000`, `838`, `100 000`, `300`, `(1993)`, `(2016)`. Elles ne doivent pas être appliquées.
- **Quatre points** sont à plus de 1 500 km du lieu dont le nom est celui de la bataille : Mondovì, Friedland, Eylau, Vitoria. Friedland et Eylau partagent le même secteur catalan, vers 42° N, 2,8° E.
- **51 lieux** portent un nom connu sans être le théâtre de l’événement. Le duché de Valmy en est le cas net : aucune coordonnée de champ de bataille, aucune année 1808 proposée pour une ligne classée bataille.
- **168 événements** restent des `historical_fact`. Les détecteurs de type et de titre n’existent pas. Leur silence ne valide pas ces classes.
- Une ligne « Wagram » stockée vers **1780**, année inférée, ne produit aucun signal. Le critique n’a pas vu 1809 dans la phrase analysée. C’est la preuve qu’un événement sans alerte peut être faux.

## 2. Statistiques

### A — Vue générale

| Mesure | Événements | Constats |
| --- | ---: | ---: |
| Actifs et examinés | 465 | — |
| Sans signal | 285 | — |
| Avec au moins une proposition | 89 | 91 propositions |
| Relecture, sans proposition retenue | 91 | 112 constats de relecture |
| Preuves insuffisantes au sens du statut | 0 | 0 |
| Plusieurs constats sur la même ligne | 23 | 203 constats au total |

Les quatre statuts d’événement sont exclusifs et totalisent 465. Les 180 événements « avec anomalie » sont la somme des 89 propositions et des 91 relectures. Les familles comptent des constats, pas des événements : date 145, lieu 58, type 0, titre 0, doublon 0.

Le statut `insufficient_evidence` reste à zéro parce que le détecteur ne l’émet que lorsqu’une phrase donne une année et que la ligne n’a aucune date stockée. Presque toutes les lignes ont une année. L’insuffisance documentaire est portée par la couverture, plus bas, et non par ce statut.

### B — Chronologie

| Précision stockée | Événements |
| --- | ---: |
| Jour | 0 |
| Mois | 0 |
| Année exacte | 457 |
| Approximative, inférée | 8 |
| Intervalle | 0 |
| Inconnue | 0 |

Les huit dates inférées sont des surfaces du type `~1780 (inferred from paragraph_context)` ou `~1784 (inferred from lifespan_education)`. L’une d’elles date une suite de Wagram de 1780.

| Nature du constat de date | Constats | Effet |
| --- | ---: | --- |
| `precision_coarser_than_passage` | 67 | Enrichissement : même année, jour déjà dans la phrase |
| `stored_time_contradicts_passage` | 17 | Proposition de changer l’année |
| `day_not_tied_to_a_year` | 32 | Jour sans année dans la phrase, aucune proposition |
| `date_association_ambiguous` | 25 | Plusieurs dates, aucune proposition |
| `date_belongs_to_another_action` | 4 | L’année appartient à une autre action, aucune proposition |

Parmi les 17 changements d’année proposés, la lecture des titres suffit à en écarter une partie :

| Ligne | Stocké | Proposé | Lecture |
| --- | --- | --- | --- |
| Ulm, « 2,000 French casualties » | 1805 | 2000 | Nombre de pertes |
| Iéna, notice « (1993), Jena 1806 » | 1806 | 1993 | Année d’édition |
| Tilsit, « not seen in over 300 » | 1807 | 0300 | Quantité |
| Leipzig, fragment « 180 » | 1812 | 0180 | Nombre |
| Leipzig, « 100 000 morts » | 1812 | 0100 | Nombre |
| Bayonne, « 838 men » | 1814 | 0838 | Pertes |
| Waterloo, jeu « (2016) » | 1815 | 2016 | Année d’édition |

Le parseur retire les signes d’un jeton et accepte tout entier de 100 à 2100. `2,000` devient 2000. Ces sept propositions sont des artefacts.

Les dix autres méritent une lecture humaine, pas une écriture automatique. Plusieurs phrases contiennent deux années et le détecteur ne garde que la première lorsqu’aucun mois n’est nommé : campagne du Rhin « 1796 » et « late 1797 » ; paludisme « à partir de 1786 … jusqu’en 1796 » ; décorations « de 1805 à 1811 » ; crises « de 1807 et de 1810-1811 ». Remplacer l’année stockée détruirait l’autre borne.

Vingt-quatre batailles n’ont qu’une proposition d’affinage, et la phrase donne le jour : Montenotte 12 avril 1796, Millesimo 13-14 avril 1796, Mondovì 21 avril 1796, Lodi 10 mai 1796, Lonato 3-4 août 1796, Rovereto 4 septembre 1796, Bassano 8 septembre 1796, Valvasone 16 mars 1797, Marengo 14 juin 1800, Iéna 14 octobre 1806. Ce sont des enrichissements, à condition que la phrase décrive bien cette bataille.

### C — Géographie

| Situation | Événements |
| --- | ---: |
| Point stocké | 428 |
| Sans point | 37 |
| Identité exacte unique | 268 |
| Identité approximative | 4 |
| Identité de centroïde | 92 |
| Aucune identité fiable | 101 |
| Libellé de lieu vide | 14 |
| Libellé sans identité | 87 |
| QID de lieu déjà stocké | 374 |
| Libellé qui est lui-même un QID | 13 |

Les 92 centroïdes ont tous un point. Un centroïde ne justifie pas un lieu de bataille, et le critique ne propose pas de le remplacer. Le point reste pourtant affichable.

Homonymie : aucun constat `place_identity_ambiguous` ni `place_precision_insufficient` sur ce corpus. L’ambiguïté rencontrée est celle du rôle, pas celle de deux QID.

| Propositions de coordonnées | Écart | Décision recommandée |
| --- | ---: | --- |
| Mondovì, Q50775, 44,38° N 7,82° E | 2 238 km | Relecture prioritaire. Le point stocké est 59,60° N 29,67° E. |
| Friedland, Q13431 | 1 926 km | Relecture prioritaire. Point stocké 41,94° N 2,81° E, en Catalogne. |
| Eylau, Q13422 | 1 900 km | Même secteur catalan, 42,03° N 2,74° E. Erreur de résolution partagée. |
| Vitoria, Q8714 | 1 540 km | Le point stocké est en Saxe, 51,29° N 13,53° E. La phrase de date décrit une autre action : ne pas appliquer le couple date et lieu ensemble. |
| Ratisbon et Ratisbonne, Q2978 | 1,4 km | Sous le seuil d’une erreur historique. Le rayon de 500 m est plus strict que l’écart entre deux points de la même ville. |
| Salamanque, Q15695 | 0,6 km | Le point est déjà celui des Arapiles. Ne pas le déplacer. |

Cinquante et un constats `place_is_not_the_event_locus` refusent une correction de coordonnées. Le nom est dans le texte, le rôle n’est pas le théâtre : duché de Valmy, décret de Berlin, mentions de Rome, garnison de Nice. C’est le comportement voulu. La contrepartie est qu’un vrai théâtre mal tourné peut aussi être retenu : « landed at Alexandria on 1 July » reste en relecture, sans proposition de point.

### D — Classification

| Type | Événements |
| --- | ---: |
| battle | 190 |
| historical_fact | 168 |
| office | 17 |
| diplomatic | 15 |
| arrival | 15 |
| residence | 10 |
| marriage | 9 |
| meeting | 8 |
| publication | 6 |
| education | 5 |
| departure | 5 |
| travel | 4 |
| exile | 4 |
| siege | 2 |
| birth, death, burial, trial, health_event, life_event, passage | 1 chacun |

`historical_fact` représente 36 % du corpus. Huit titres sont de la forme `office · 1804` ou `marriage · 1796`. Treize libellés de lieu sont un QID nu. Cinq titres de `historical_fact` contiennent les mots bataille ou siège. Des notices bibliographiques et un jeu de 2016 sont stockés comme des faits ou des batailles.

Les familles type et titre n’ont pas de détecteur. Leur compte à zéro ne valide ni le type ni le titre. Le détecteur de doublon, lui, a tourné : aucun `occurrence_key` actif n’est répété dans les 465 lignes. Ce zéro-là est un résultat, pas une absence d’outil.

### E — Couverture documentaire

| Niveau | Événements | Lecture |
| --- | ---: | --- |
| `bounded` | 266 | Passages chargés tous analysés, et identité fiable si un lieu est nommé |
| `partial` | 199 | Fenêtre trop courte, ou lieu sans identité fiable |
| `none` | 0 | Aucun événement actif n’est sans passage |

6 163 passages sont disponibles, 2 631 ont été analysés. 149 événements dépassent la fenêtre de 12 passages. Chaque événement conserve la liste de ses preuves, y compris celles qui n’ont pas été lues. `certifies_historical_truth` est faux pour les 465.

Répartition croisée : 156 sans signal et couverture bornée ; 129 sans signal et couverture partielle ; 51 propositions à couverture bornée ; 38 propositions à couverture partielle ; 59 relectures bornées ; 32 relectures partielles. Les 129 lignes silencieuses et partielles sont le groupe qu’il ne faut pas lire comme « vérifié ».

## 3. Vingt anomalies prioritaires

Chaque ligne reste une recommandation. La justification est le titre stocké et le constat du détecteur. Quand le nombre n’est pas une année, la recommandation est de ne rien écrire.

| | Identifiant | Type | Constat | Proposition | Suite |
| --- | --- | --- | --- | --- | --- |
| 1 | `7b618070-7668-4102-9dea-5c3937c25b8c` | battle | Mondovì, 1796, point à 2 238 km | Q50775 et jour `1796-04-21` | Relire le point. Le jour est dans le titre. |
| 2 | `6f2c76d3-0065-4590-8301-63828e6e4f32` | battle | Friedland, point catalan, « 14 June » sans année dans la phrase | Q13431 seulement | Relire le point. Ne pas inventer l’année. |
| 3 | `9fa1ed25-31ac-48e1-b2da-7c5738d3a79c` | battle | Eylau, même secteur catalan | Q13422 | Même résolution géographique que Friedland. |
| 4 | `8474fcf5-cbaa-4428-bbb8-db051bc5172b` | battle | Vitoria, point en Saxe ; la date de la phrase est une autre action | Q8714, date refusée | Ne pas appliquer le lieu tant que l’action n’est pas séparée. |
| 5 | `3f71c2a5-6355-4cec-afb8-37606b31802f` | battle | Wagram stocké `~1780`, inféré. Aucun constat. | Aucune | Lacune de méthode. Le titre décrit le traité après Wagram. |
| 6 | `cae82592-49c9-481d-adf1-17b82ff23dfa` | battle | Titre « Duke of Valmy », stocké en 1792, classé bataille | Aucune coordonnée, aucune année 1808 | Abstention correcte. La ligne mélange le titre de 1808 et l’année de la bataille. |
| 7 | `6c77a950-72c4-492f-8ea9-da478a00094b` | historical_fact | La phrase dit l’abolition en décembre 1805 et l’introduction en 1793. Stocké : 1793. | 1805 | Deux faits. Trancher le sujet de la ligne avant d’écrire. |
| 8 | `fb2a8620-ae08-409e-ae22-f63503fefd77` | historical_fact | Paludisme « à partir de 1786 … jusqu’en 1796 », stocké 1796 | 1786 | En faire un intervalle, pas un remplacement. |
| 9 | `0aa7d1d6-8b3e-4ab5-8970-d9267ad08ccc` | residence | « moved to mainland France in 1779 » et une commission, stocké 1785 | 1779 | Les deux dates sont dans le même récit. |
| 10 | `ca009754-e2b5-4e33-b816-13fa2f829f97` | battle | Rhin 1796 et paix « by late 1797 », stocké 1797 | 1796 | Le premier jeton d’année l’a emporté. Ne pas appliquer. |
| 11 | `af8372d6-2550-4f51-ae64-627823062f11` | historical_fact | Traités de Tilsit, stocké 1805, lieu seulement mentionné | 1807, pas de point | La date peut être la bonne ; le rôle du lieu a été respecté. |
| 12 | `999f79cf-311a-4cdd-acca-e1c844211ea2` | historical_fact | « De 1805 à 1811 », décorations, stocké 1811 | 1805 | Borne d’un intervalle. |
| 13 | `598a1cc9-e247-4bd9-95c8-1d35da3cf514` | historical_fact | « crises de 1807 et de 1810-1811 », stocké 1811 | 1807 | Même motif. |
| 14 | `f1a657ba-7821-4dbf-b9c9-f825ec5a4311` | siege | Siège de Saint-Sébastien, 1813 ; la phrase proclamme Joseph roi en 1808 | 1808 | Autre action. Ne pas transférer l’année. |
| 15 | `f00ec150-906f-487a-b4af-e31908a0daf9` | arrival | « the Napoleon of South America », Bolívar, Chuquisaca, 1819 | `1825-04-25` | Mauvais sujet. Ne pas dater Napoléon avec cette phrase. |
| 16 | `03106365-2852-4c5e-a57c-20ee91778e3b` | battle | Ulm, « 2,000 French casualties » | 2000 | Artefact. Ne pas appliquer. |
| 17 | `b22cc894-1a5f-4e74-918c-3667af4be32a` | battle | Notice Osprey « (1993), Jena 1806 » | 1993 | Année d’édition. |
| 18 | `d3fe9798-ceb6-4109-bc8a-6636adcab702` | historical_fact | Tilsit, « over 300 » ; le lieu est une mention | 0300 | Artefact numérique. Le refus de coordonnées est juste. |
| 19 | `6ba6cda6-f1cd-4b6e-9942-ef6aeb61ee27` et `6179e870-954a-44fd-a654-3fc136729f39` | historical_fact, battle | « 100 000 morts » et un fragment « 180 », Leipzig | 0100 et 0180 | Artefacts. |
| 20 | `897bfdce-f515-4c0a-b631-f4ad4a104338`, `d49e0cf0-fb9e-4a23-a64d-a2c5b3c8cced`, `6717c6f4-7157-44b5-ba0f-fa123bc6b48c` | arrival, historical_fact, life_event | Pertes « 838 », jeu Waterloo « (2016) », Farington stocké 1802 et proposition 1812 | 0838, 2016, 1812 | Ne pas appliquer. Le titre de Farington dit déjà 1802. |

L’identifiant `cae82592-49c9-481d-adf1-17b82ff23dfa` a pour titre « In 1808 he was ennobled by Napoleon and became Duke of Valmy » et pour année stockée 1792. Les constats sont `place_is_not_the_event_locus` et `date_belongs_to_another_action`. Le critique n’a pas déplacé le point sur le champ de bataille et n’a pas remplacé 1792 par 1808.

## 4. Vingt événements qui demandent du travail documentaire

Le statut « preuves insuffisantes » est vide. Le travail ci-dessous est donc classé par motif réel : fenêtre de 12 passages, identité absente, titre générique, date inférée, ambiguïté. Une ligne sans signal et à couverture partielle n’a pas été vérifiée.

| | Identifiant | Motif | Données actuelles | Constat |
| --- | --- | --- | --- | --- |
| 1 | `3f71c2a5-6355-4cec-afb8-37606b31802f` | Date inférée, couverture partielle | Bataille, `~1780`, lieu « La Bataille De Wagram », 56 passages dont 12 lus | Aucun signal. Le titre parle du traité après Wagram. |
| 2 | `d3ab0bf2-b38c-4623-8003-11fc9be29818` | Fenêtre | Waterloo, année 1815, 59 passages dont 12 lus | Proposition de jour seulement. Le reste des preuves n’a pas été lu. |
| 3 | `1e1ee54b-e626-4430-a2be-887c980fabb9` | Fenêtre et bibliographie | Arapiles, 1812, 59/12 | Le titre est une contribution historiographique. L’écart de point est de 0,6 km. |
| 4 | `49f9ee29-e85f-4316-b384-be8e89d1c44c` | Sans lieu fiable | Année 1814, 55/12, pas de libellé | Notice « Maurice Bois… lieutenant d’artillerie à Auxonne ». Aucun signal. |
| 5 | `c43d38e8-5439-40c5-9973-7bc74caa907a` | Couverture partielle | `~1809` inféré, lieu « Support Bessières », 37/12 | Le titre envoie Lannes à Neumarkt. Le libellé de lieu n’est pas un lieu. |
| 6 | `3386b9b1-8773-4634-bd27-f7046ed55b31` | Identité absente | Rovereto, 4 septembre 1796, 45/12 | Le jour est proposé. Le lieu n’a pas d’identité fiable. |
| 7 | `f00ec150-906f-487a-b4af-e31908a0daf9` | Ambiguïté de sujet | Arrivée à Chuquisaca, 1819 | Phrase sur Bolívar, « Napoleon of South America ». |
| 8 | `6717c6f4-7157-44b5-ba0f-fa123bc6b48c` | Libellé inutilisable | `life_event`, lieu « sideways », 1802 | Proposition 1812 non fiable. Le portrait de Farington est daté 1802 dans le titre. |
| 9 | `4b2e2ff6-6467-460a-910b-7b82c6627357` | Ambiguïté, un seul passage | Siège, lieu « Pie VII », 1813 | Concordat de Fontainebleau. Le « lieu » est une personne. |
| 10 | `7912a07d-8b15-428f-a32d-e12754366a7c` | Titre générique | `marriage · 1796`, lieu `Q171480`, 1 passage | Aucun récit. Le libellé est un QID. |
| 11 | `81c28730-40a3-4d84-944d-6968ea21b52f` | Titre générique | `marriage · 1810`, lieu `Q157491`, 1 passage | Même forme. |
| 12 | `2812797d-de62-428d-acd4-3d2e279d1cd8` | Titre générique | `office · 1799`, lieu `Q1131971` | Un passage, aucun signal, couverture partielle. |
| 13 | `286e125d-8205-44ab-abfb-25b2dcbf949e` | Titre générique | `office · 1800`, lieu `Q23034946` | Idem. |
| 14 | `948a49f4-ad9c-4c02-adac-19368afde877` | Titre générique | `office · 1804`, lieu `Q5373953` | Idem. |
| 15 | `29d4c811-6684-4b79-88fe-b23fb38973f8` | Titre générique | `office · 1805`, lieu `Q271148` | Idem. |
| 16 | `16c6ba97-deb5-48f6-bd70-2691d3c6b099` | Titre générique | `office · 1806`, lieu `Q19808845` | Idem. |
| 17 | `a9edd12e-ad08-4ad2-9f0e-a82acdc07b8b` | Titre générique | `office · 1815`, lieu `Q5373953` | Idem. Le même QID sert pour 1804 et 1815. |
| 18 | `1c586625-27e6-4225-80ef-d6012971340d` | Sans identité | 1798, 34/12, pas de lieu | Hypothèse « si les Français avaient gagné Waterloo ». Aucun signal. |
| 19 | `8562df19-d510-422a-82a4-e817ae2c0d26` | Libellé douteux | Wolkowisk, 1812, 34/12 | Le titre parle de Petersbourg, pas du lieu stocké. Aucun signal. |
| 20 | `636a74d5-49bd-46ac-9b65-c46ec1b95ca2` | Libellé douteux | Maya, 1813, 34/12 | Le titre parle du royaume de Joseph. Aucun signal, couverture partielle. |

## 5. Problèmes récurrents du pipeline

1. La date sémantique reste une année, même quand la phrase source contient le jour. `start_time` ne doit pas servir de date affichée ; ici la précision stockée elle-même est presque toujours `year`.
2. Une année inférée du contexte du paragraphe peut être fausse et traverser le critique si la bonne année n’est pas dans la phrase lue. Wagram `~1780` en est l’exemple.
3. Le premier nombre de trois ou quatre chiffres dans la phrase devient une année. Les pertes, les effectifs et les années d’édition produisent des propositions fausses et assurées.
4. Une phrase à deux années sans nom de mois ne déclenche pas l’ambiguïté : une seule année est gardée.
5. Le géocodeur a envoyé Friedland et Eylau au même endroit de Catalogne, Mondovì vers la Russie, Vitoria vers la Saxe.
6. Un centroïde, un QID nu, un fragment de phrase (« Support Bessières », « sideways », « Pie VII ») ou un titre honorifique peuvent porter un point, ou empêcher d’en avoir un juste.
7. Le filet `historical_fact`, les titres `office · année` et les notices bibliographiques mélangent l’événement, la source et le commentaire.
8. Une comparaison lexicale avec « Napoléon » fait entrer dans la chronologie un épisode de Bolívar.
9. La fenêtre de douze passages laisse 3 532 preuves hors de l’analyse. L’événement le plus documenté, Waterloo, a 59 preuves.

## 6. Limites de la méthode

- Revue déterministe, sans modèle. Les cas ambigus sont restés en relecture. Aucun jeton n’a été dépensé.
- Le rôle du lieu dépend de tournures (« bataille de », « duc de », « traité de », « landed at » n’en fait pas partie). Une tournure manquée produit une abstention, parfois une proposition encore trop large.
- Les détecteurs de type et de titre ne sont pas écrits.
- Le doublon n’est cherché que par `occurrence_key` parmi les lignes chargées.
- Douze passages par événement, choisis d’abord s’ils contiennent une année ou le libellé du lieu.
- L’échantillon précédent de 20 lignes, ordonné en favorisant quelques lieux connus, n’a pas servi. L’ordre de cette revue est chronologique puis par identifiant.
- « Sans signal » et couverture `bounded` ne certifient pas le fait. 156 événements sont dans ce cas ; 129 autres sont sans signal et seulement partiels.
- Les quatre grands écarts de coordonnées s’appuient sur le gazetteer interne et sur le titre. Ils restent des propositions.

## 7. Corrections recommandées, par priorité

Aucune n’a été écrite dans `canonical_events`.

**P0 — ne pas appliquer.** Les sept années numériques (2000, 1993, 0300, 0180, 0100, 0838, 2016) et la proposition 1812 sur le portrait de Farington. Les déplacements de Ratisbonne (1,4 km) et de Salamanque (0,6 km).

**P0 — relecture humaine des points.** Mondovì, Friedland, Eylau, et Vitoria seulement après avoir séparé l’action décrite de la bataille.

**P1 — lignes à retirer du récit de Napoléon, ou à reclasser, avant toute biographie.** Bolívar à Chuquisaca ; le jeu *Napoleon Saga* ; la notice Osprey de 1993 ; la notice Maurice Bois ; le duché de Valmy classé `battle`.

**P1 — dates à représenter comme des intervalles, pas comme un remplacement.** Paludisme 1786–1796, décorations 1805–1811, crises 1807 et 1810–1811, départ de Corse et commission 1779 / 1785, abolition du calendrier révolutionnaire face à son introduction.

**P1 — Wagram `~1780`.** La ligne ne doit pas entrer dans un chapitre de jeunesse. L’année est une inférence, et le titre parle du traité de 1809.

**P2 — affinages de jour,** après correction du parseur, pour les batailles dont la même phrase donne le jour et la même année : Lodi, Marengo, Montenotte, Iéna, et les autres de la liste des 24. Ce sont des précisions, pas des réfutations.

**P2 — Tilsit.** La proposition 1807 est la seule correction d’année encore plausible sans second fait contradictoire dans le titre, et le critique a refusé de déplacer le point parce que le lieu est une mention.

## 8. Feuille de route avant les biographies

1. Corriger le parseur d’années : un nombre précédé d’une virgule de milliers, une perte, une année d’édition hors de la vie du sujet, ou deux années dans la même phrase, ne produisent pas une proposition. Ils produisent une abstention.
2. Marquer les dates inférées (`paragraph_context`, `lifespan_education`) comme non narratives tant qu’une phrase ne les reprend pas.
3. Ne géocoder automatiquement que le théâtre. Un titre, un décret, une mention et un objet d’acte restent sans point, ou avec un point explicitement non événementiel.
4. Revoir le paquet catalan Friedland / Eylau et les points Mondovì et Vitoria avant d’afficher ces batailles sur une carte de chapitre.
5. Écrire les détecteurs de type et de titre. Tant qu’ils n’existent pas, un chapitre ne doit pas traiter `historical_fact` comme une catégorie comprise.
6. Remplacer les titres `office · année` et les libellés QID par le libellé de l’acte et le nom du lieu, ou les laisser hors récit.
7. Lever la fenêtre de douze passages pour les événements qui deviendront des chapitres, ou afficher « 12 passages lus sur 59 ».
8. Garder le critique en lecture seule, en amont de la publication. Une biographie par périodes, campagnes et chapitres doit consommer ces statuts. Elle ne doit pas les écraser, ni transformer une couverture partielle en épisode certain.

## 9. Vérifications de sécurité

| Contrôle | Résultat |
| --- | --- |
| Événements actifs Q517 | 465 |
| Examinés | 465, supérieur au plafond d’échantillon de 20 |
| Empreinte `canonical_events`, `event_candidates`, `event_evidence` | Identique avant et après |
| Comptes de ces trois tables | 3 515, 6 239, 47 022, identiques |
| Transaction de lecture | `SET TRANSACTION READ ONLY` sur le chargement. Le test d’intégration montre qu’un `UPDATE` dans cette transaction échoue. |
| Journal | Une ligne de run, une tâche, second passage idempotent, `writes = false` |
| Jetons et coût | 0 et 0 |
| Collecte documentaire | Aucune |
| Corrections appliquées | Aucune |

Les comptes et l’empreinte portent sur toutes les lignes des trois tables, pas seulement sur Q517. Une modification de valeur, même sans changement de nombre de lignes, aurait changé l’empreinte.
