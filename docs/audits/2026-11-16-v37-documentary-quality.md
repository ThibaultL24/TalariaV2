# V3.7 — qualité documentaire et ingestion bornée

Mesure du 16 novembre 2026, branche `feat/v37-documentary-quality`, partie de `bench/historical-engine` (`e05f2ec`). Aucune écriture dans `talaria_engine_development`. Migration 055 non appliquée. 053 et 054 sont en place. Aucun chapitre créé, aucun événement canonique modifié, aucune preuve supprimée.

Le succès cherché est un fait nouveau, daté, situé et porté par une phrase. Une notice de catalogue ne compte pas.

## Ce qui a été corrigé

Une citation qui commence par `[HAL]`, `[OPEN_ALEX]`, `[GALLICA]`, `[PERSEE]`, `[THESES` ou `[BNF]` est une notice. Si elle ne nomme pas le lieu de l'événement, elle est hors sujet. Si elle le nomme, elle reste une notice, pas une preuve. Un titre nu (« Bibliographie du combat de Rivoli ») est un contexte. La phrase « The Battle of Lonato was fought on 3 and 4 August 1796 » reste une preuve directe.

Cette lecture est dans `documentary_relation`. Le rattachement d'une preuve à un événement déjà connu refuse désormais une notice. Le candidat reste en revue. Les lignes `event_evidence` déjà stockées ne sont ni mises à jour ni effacées.

Le rapprochement catalogue ne s'accroche plus au mot « battle » ni à « Napoleon ». Il ne retient qu'un lieu précis. Un livre sur les guerres napoléoniennes ne se lie plus à Lonato, Millesimo, Mondovì et Rivoli. Le test SQLx `catalog_metadata_never_creates_event_evidence` confirme qu'un candidat de document ne crée pas de preuve.

Le lexique des ingestions futures reconnaît `naît`, `est élu`, `publia`, `découvrit` et `rencontra`. `demeure au` reste une résidence, et Versailles garde son lieu. Les 1 175 `historical_fact` existants ne sont pas reclassés.

## Baseline, relue sans écriture

| Mesure | Valeur |
|---|---|
| Événements `person` actifs | 2 218 |
| Inactifs | 1 299 |
| `historical_fact` actifs | 1 175 |
| Anecdotes de type canonique | 5 |
| Anecdotes validées pour publication | 0 |
| Chapitres en base | 0 |
| Q517 Napoléon | 465 actifs, dont Lodi, Montenotte, Millesimo, Mondovì, Rivoli |
| Q535 Victor Hugo | 208 |
| Q7186 Marie Curie | 74 |
| Q762 Léonard | entité, 0 événement |
| Q868 Aristote | aucune entité |
| Q466987 Marceline Desbordes-Valmore | aucune entité |

Lonato, Castiglione, Mantoue, Cherasco et Campo Formio n'ont pas de `place_label` sur Q517. Identités vérifiées sur Wikidata avant l'essai : Q517 Napoleon, Q535 Victor Hugo, Q7186 Marie Curie, Q762 Leonardo da Vinci, Q868 Aristotle, Q466987 Marceline Desbordes-Valmore.

## Preuve et notice

Les batailles de Q517 portent des milliers de lignes `llm_quote`. La phrase Wikipédia décrit le combat. Les lignes HAL, OpenAlex et Gallica sont souvent le titre entre crochets. Ce format n'est plus produit par le code actuel. Le correctif empêche le prochain rattachement. Il ne réécrit pas l'archive.

Sur l'essai live, aucun titre HAL ou OpenAlex n'a été classé comme passage direct. Exemples :

- HAL, requête « montenotte » : « Lawsonite et aragonite dans les unités océaniques de la zone du Combin ». Hors sujet.
- OpenAlex, « Napoleon » : « Napoleon the Great », « Napoleon at St. Helena ». Notices générales, pas la phrase de Lonato.
- HAL, « Victor Hugo » : articles sur la réception de l'œuvre. Contexte bibliographique.
- OpenAlex, « Leonardo da Vinci » : « The Notebooks of Leonardo da Vinci ». Notice, pas un épisode daté.

Gallica, Persée, theses.fr et la BnF n'ont pas été appelées. Aucun modèle payant. Coût : 0.

## `historical_fact`

1 110 des 1 175 titres n'ont aucun indice verbal unique. Un mot-clé reclasserait à tort : « ornée de deux toiles » contient « née ». Propositions, non appliquées :

| Phrase | Personne | Lecture | Décision |
|---|---|---|---|
| François-Victor naît en octobre 1828 | Victor Hugo | Naissance du fils | Solide comme naissance d'un autre, pas comme naissance de Hugo |
| Barack Obama est élu au Sénat de l'Illinois en 1996 | Barack Obama | Mandat | Solide, non appliqué |
| She was appointed director of the Curie Laboratory | Marie Curie | Nomination | Solide, non appliqué |
| In 1673, Louis Jolliet and Jacques Marquette discovered the Mississippi | Louis XIV | Découverte d'autrui | Injustifié sur Louis XIV |
| De Gaulle and the Anglo-Saxons (Viking Press, 1970) | Charles de Gaulle | Notice de livre | Ce n'est pas un fait ; la ligne n'est pas supprimée |
| Le poète y occupe une chambre ornée de toiles de Manet | Baudelaire | Résidence possible, indice « née » trompeur | Incertain |

## Anecdotes

Le détecteur sépare un épisode singulier, un grand événement, et un récit que le texte dit apocryphe. La lettre la plus courte de Hugo reste un récit non établi. Une bataille ne devient pas une anecdote. Rien n'est validé. Rien n'est inventé.

Les cinq lignes de type `anecdote` déjà en base n'ont pas été modifiées. La mort de Léopoldine est déjà un fait stocké. Elle est hors de la fenêtre de 20 000 caractères de cet essai, donc le détecteur ne l'a pas revue.

## Jeu de référence

`fixtures/benchmarks/v37-reference.json` est figé au 16 novembre 2026. Il n'a pas été modifié après les scores. Il contient des faits directs, des dates imprécises, un doublon (le pont de Lodi), des hors-sujets, un mauvais lieu, une date partielle, un fait sans point (Mombello), des récits légendaires, et des faits absents du corpus mais cherchables (Cherasco, les Élégies de 1819).

## Essai live

Wikipedia en texte brut, tronqué à 4 000 caractères pour une bataille et 20 000 pour une biographie. HAL et OpenAlex : 5 titres. Wikidata : contrôle des six QID. Pas de base, pas de persistance, pas de modèle. Durée : 65 secondes. La page « Battle of Arcole » n'a pas répondu. L'appel HAL « leonardo da vinci » a échoué une fois.

Une même phrase de Montenotte sort trois fois : lieu « Montenotte », lieu « Montenotte Superiore (then », lieu « Sardinia ». Millesimo produit aussi le lieu « One Of A Series Of Small Battles That Were Fought ». Ce sont des lieux faux sur un fait vrai. Ils ne comptent pas comme des faits nouveaux.

## Tableau 1 — qualité du moteur

| Personne | Documents | Pertinents | Passages probants | Candidats uniques | Faits nouveaux fiables | Faux positifs de piège | Dates et lieux | Temps, coût |
|---|---|---|---|---|---|---|---|---|
| Napoléon | 10 pages, 6 notices | 8 pages de campagne | Montenotte, Millesimo, Mondovì, Lodi, Lonato, Castiglione, Mantoue | 19 | 3 : Lonato, Castiglione, Mantoue | 0 | L'année est bonne. Le lieu se dégrade sur la même phrase | part des 65 s, 0 |
| Victor Hugo | 1 page, 16 notices | la biographie jusqu'au caractère 20 000 | naissance, Contemplations, Misérables | 46 | 0 : déjà dans les 208 | 0 | Besançon est daté. Jersey et Guernesey sont dans la fenêtre sans l'année exigée | 0 |
| Marie Curie | 1 page, 11 notices | la biographie | Nobel 1903, mort de Pierre, mort de Marie | 31 | 0 : le polonium de juillet 1898 est déjà stocké, hors de ce que l'essai a retenu | 0 | Varsovie sort sans 1867 dans la phrase de naissance | 0 |
| Léonard | 1 page, 5 notices OpenAlex | la biographie | naissance, Joconde | 43 | 2, non écrits : naissance, Joconde | 0 | La Cène, Amboise et la mort ne sont pas sortis | 0 |
| Aristote | 1 page, 10 notices | la biographie | Stagire 384 av. J.-C., mort | 17 | 2, non écrits : naissance et mort | 0 | Le Lycée et Mieza ne sont pas sortis. Les dates restent approximatives | 0 |
| Desbordes-Valmore | 1 page anglaise, 14 notices | notices de réception, pas le voyage | les Élégies, année 1819 | 9 | 1 piste, non écrite | 0 | « born in Douai » et la mort de la mère n'ont pas l'année dans la phrase anglaise | 0 |

« Fait nouveau » veut dire : le passage correspond au jeu de référence, et le corpus actif n'a pas déjà ce lieu pour cette personne. Un fait présent dans plusieurs extracteurs compte une fois. Quarante notices ne valent pas une phrase.

## Tableau 2 — richesse des biographies

| Personne | Déjà en base | Déjà utilisables sur la carte | Anecdotes candidates | Validées | A | B | C | Après l'essai |
|---|---|---|---|---|---|---|---|---|
| Napoléon | 465 | 261 | 0 nouvelle | 0 | 2 lectures déjà posées (campagne, pouvoir) | 2 (déplacements, famille) | lacune Cherasco, Mombello | Lonato, Castiglione et Mantoue peuvent renforcer la campagne. Aucun chapitre créé |
| Victor Hugo | 208 | 54 | la mort de Léopoldine est déjà un fait, pas une validation | 0 | 0 | 3 (résidences, publications, mandats) | 1 (anecdotes, dont l'apocryphe) | Jersey et Guernesey sont déjà en base, parfois avec un lieu illisible. Rien de nouveau |
| Marie Curie | 74 | 19 | 0 | 0 | 0 | 1 (études) | 2 (publications, scandale de 1911) | Le polonium est déjà écrit. L'essai ne l'a pas retrouvé dans sa fenêtre |
| Léonard | 0 | 0 | 0 | 0 | 0 | 0 | 1 piste : naissance et Joconde | Pas un chapitre. La Cène et Amboise manquent |
| Aristote | 0 | 0 | 0 | 0 | 0 | 0 | 1 piste : naissance et mort | Le Lycée n'est pas un chapitre |
| Desbordes-Valmore | 0 | 0 | le voyage et la fièvre jaune sont dans la page française, pas datés en anglais | 0 | 0 | 0 | 1 piste | Pas un chapitre |

A : prêt pour une revue. B : brouillon crédible. C : piste à enrichir. Aucun D n'a été écrit. Un tas de `historical_fact` n'est pas un chapitre. Aucune de ces lectures n'est publiée.

## Avant et après

Il n'y a pas de second essai live. Les chiffres du tableau 1 sont ceux du code actuel. Le gain mesuré par les tests, pas par un volume, est celui-ci :

- Une notice de Lonato, Millesimo, Mondovì ou Rivoli n'atteste plus le combat.
- Un livre qui dit seulement « battle » ou « Napoleon » ne se lie plus aux quatre combats.
- Une résidence « demeure au château de Versailles » garde Versailles.
- Les pièges (pénicilline, bicyclette, Euripe, Moscou, couronnement de 1796) ne sont pas sortis : 0.

L'extraction anglaise, elle, n'a pas progressé dans cet essai : les trois extracteurs répètent la même phrase, et le lieu casse.

## Cinq corrections suivantes

1. Étiqueter en lecture les `llm_quote` déjà stockées qui sont des notices, sans les effacer. L'archive reste. Le compteur de preuve directe ne les compte plus.
2. N'émettre qu'une fois une phrase, et refuser un lieu qui est une proposition (« One Of A Series Of Small Battles »).
3. Reprendre l'année de la phrase voisine quand le lieu est dans celle-ci. Jersey et Douai sont écrits, l'année est à côté.
4. Lire la biographie dans la langue qui porte la date. La page française de Desbordes-Valmore donne le 20 juin 1786 et mai 1802. La page anglaise tronquée ne les donne pas.
5. Exiger le nom de l'événement dans la notice, pas seulement un token partagé. « Montenotte » ne doit pas ramener une roche des Alpes. « Rivoli » ne doit pas ramener Louvre-Rivoli.

## Tests

- `talaria-quality` : 7 tests documentaires, dont les quatre combats et Versailles.
- `talaria-judge` : les phrases `naît`, `publia`, `découvrit`, et la résidence de Versailles.
- `talaria-sources` `life_trace_extract` : 9, dont le lieu de Versailles.
- `talaria-api` : le livre napoléonien ne matche pas les quatre batailles ; un titre qui nomme Rivoli reste un candidat de contexte ; le test SQLx confirme qu'aucune preuve n'est créée.
- Sonde live ignorée par défaut : 1 essai, 65 s, coût 0.
- Le frontend n'a pas été modifié.

## Limites

Pas d'ingestion complète, pas de base éphémère écrite, pas d'appel Gallica, Persée, theses.fr, BnF, ni de modèle payant. Arcole n'a pas été téléchargé. La fenêtre biographique s'arrête à 20 000 caractères : Léopoldine et le retour de 1870 sont au-delà. Le jeu de référence n'a pas été ajusté pour relever les scores.
