# V3.8 — déduplication, ancrage et second essai

Mesure du 16 novembre 2026, branche `feat/v38-historical-dedup`, partie de `feat/v37-documentary-quality` (`d11d194`). Aucune écriture dans `talaria_engine_development`. Migration 055 non appliquée. Aucun événement canonique modifié, aucune fusion, aucun chapitre publié. Coût : 0. Aucun modèle payant.

Le fichier `2026-11-16-v37-live-probe.json` n'a pas été réécrit. Les chiffres V3.7 ci-dessous sont ceux de ce fichier et du rapport V3.7.

## Tableau demandé

| Indicateur | V3.7 | V3.8 |
|---|---|---|
| Découvertes candidates | 8 | 2 batailles absentes du corpus, plus 1 bataille sans année |
| Faits effectivement distincts | Lonato était déjà en base | 2 : Castiglione (1796) et Mantoue (1797), non écrits |
| Nouveaux faits pour Napoléon | 3 annoncés | 2 confirmés absents ; Lonato est un doublon |
| Anecdotes validées | 0 | 0 |
| Chapitres prêts pour revue | 2 | 2 |
| Erreurs géographiques | Sardaigne prise pour le théâtre | 1 lieu-royaume écarté ; Sardaigne ne localise plus Montenotte |
| Doublons non reconnus | Lonato compté comme nouveau | 0 sur les batailles datées du jeu ; Bassano sans année reste incertain |

Le critère n'est pas atteint comme une hausse de volume. Il est atteint comme une baisse des faux nouveaux : huit propositions deviennent deux faits absents, un doublon expliqué, et une ambiguïté explicite.

## Les huit propositions de V3.7

| Fait annoncé | Personne | Source de l'essai | Décision | Pourquoi |
|---|---|---|---|---|
| Lonato, août 1796 | Napoléon | Wikipédia, « The Battle of Lonato was fought on 3 and 4 August 1796 » | Doublon | Déjà canonique. Le lieu stocké est « Italy », le titre contient Lonato. |
| Castiglione, 1796 | Napoléon | Wikipédia, Battle of Castiglione | Nouveau, non écrit | Aucune ligne active Q517 dont le titre ou le lieu contient Castiglione. |
| Mantoue, 1796–1797 | Napoléon | Wikipédia, Siege of Mantua | Nouveau, non écrit | Aucune ligne Mantua ni Mantoue sur Q517. L'essai retient l'année 1797. |
| Naissance de Léonard | Léonard | Wikipédia anglaise | Absent du corpus, non réécrit | Q762 a une entité et 0 événement. Rien n'a été persisté. |
| La Joconde | Léonard | Wikipédia anglaise | Absent du corpus, non réécrit | Même situation. La Cène, Amboise et la mort manquaient déjà en V3.7. |
| Naissance d'Aristote | Aristote | Wikipédia anglaise | Incertain | V3.7 a sorti Stagire avec l'année −322 et le lieu Athènes. Ce rattachement est maintenant refusé. |
| Mort d'Aristote | Aristote | Wikipédia anglaise | Absent du corpus, non réécrit | Aucune entité Q868. Non persisté. |
| Élégies, 1819 | Desbordes-Valmore | Wikipédia anglaise | Piste déjà vue | Publication, pas une anecdote. Non écrite. |

Aucune de ces lignes n'a été fusionnée ni insérée.

## Lonato

Lecture seule sur le corpus actif :

- Identité logique `472a66c3-2775-45cd-9660-fcaf7afb70a8`
- Version active `07764eba-3f83-4a1f-a737-a556caf4ce8f`
- Type `battle`, année 1796, `place_label` exactement `Italy`
- Le titre commence par « The Battle of Lonato was fought on 3 and 4 August 1796 »

V3.7 cherchait `place_label ILIKE '%lonato%'`. Le lieu stocké est le pays, pas la ville. Le titre, lui, nomme Lonato. Ce n'est pas une seconde bataille, ni une date affinée, ni un autre lieu d'action. C'est un échec du protocole de comparaison.

Le rapprochement V3.8 compare le type, l'année, et le nom qui suit « Battle of » dans le titre connu, pas seulement `place_label`. Lonato 1796 ressort `Duplicate`. Castiglione, absent de la liste connue et du corpus, ressort `New`. Une même place sans année ressort `Uncertain` : Bassano sans année n'est pas une seconde bataille de Bassano. Bassano daté de 1796, déjà stocké avec le lieu « Bassano », ressort `Duplicate`.

Deux événements distincts ne sont pas fusionnés. Le moteur propose le rapprochement et s'arrête.

## Ce qui a changé dans l'extraction

Une proposition est `type|année|radical du lieu`. Montenotte et « Montenotte Superiore » partagent cette clé. Trois extracteurs sur la même phrase n'ajoutent qu'une proposition. Deux lieux différents dans une même phrase restent deux propositions. Un combat sans lieu ancré ne s'écrase pas sur les autres combats sans lieu : la clé retombe alors sur la position dans le texte.

Pour une bataille ou un siège, le lieu retenu est celui qui suit « battle of », « bataille de », « siege of » ou « siège de ». « near » ne sert que si le nom n'est pas un royaume. « Kingdom of Sardinia » ne devient pas le théâtre. Un voyage vers la Sardaigne garde l'île.

Une naissance ou une mort n'hérite plus l'année de la phrase précédente. « He was born in 384 BC in the city of Stagira » reste 384 et Stagira, même si la phrase d'avant dit la mort à Athènes en 322. Une naissance sans année est laissée vide plutôt que datée par le décès. « née le … à Douai » garde Douai sans gazetteer. « ornée » ne contient pas le motif « née le » et n'est pas une naissance. Une publication n'hérite pas Guernesey. Versailles, dans « demeure au château de Versailles », reste Versailles. Le traité des Pyrénées de 1659 reste un événement diplomatique, sans recevoir le lieu de la phrase précédente.

« Louvre-Rivoli » est un seul jeton. Il ne satisfait pas le foyer « Rivoli ». Une notice `[HAL]` ou `[OPEN_ALEX]` n'atteste toujours pas l'événement.

## Second essai

Même jeu, mêmes plafonds : 4 000 caractères par page de bataille, 20 000 pour une biographie. Une page française supplémentaire pour Marceline Desbordes-Valmore, parce que la fenêtre anglaise de V3.7 n'avait pas la date de naissance. Cette page empêche une comparaison stricte pour elle seule. HAL et OpenAlex n'ont pas été rappelés : la comparaison catalogue repose sur les tests, pas sur un second échantillon live. Durée : 18 secondes. Onze pages napoléoniennes ont répondu, y compris Arcole, qui avait échoué en V3.7.

| Personne | V3.7 lignes / uniques | V3.8 lignes / uniques | Lecture |
|---|---|---|---|
| Napoléon | 25 / 19, 10 pages | 27 / 14, 11 pages | Les répétitions baissent. Arcole est lu, aucune bataille d'Arcole n'est retenue. |
| Victor Hugo | 48 / 46 | 43 / 38 | Moins de copies. Dix passages candidats, aucun validé. |
| Marie Curie | 35 / 31 | 30 / 28 | Le mémoire sur le polonium n'est plus compté comme une anecdote. |
| Léonard | 47 / 43 | 38 / 36 | La phrase sur Caterina sort deux fois. |
| Aristote | 21 / 17 | 9 / 8 | Les dates héritées à tort disparaissent. Le rappel des naissances non datées baisse. |
| Desbordes-Valmore | 9 / 9, anglais seul | 10 / 6, anglais + français | La page française contient 1786 et Douai. |

Décisions napoléoniennes de cet essai : Montenotte, Millesimo, Mondovì, Lodi, Lonato et Bassano 1796 sont des doublons. Castiglione 1796 et Mantoue 1797 sont nouveaux et non écrits. Bassano sans année est incertain. Un lieu de royaume a été écarté.

## Anecdotes candidates

Aucune n'est validée. Le texte dit « usually identified » ou « would have » reste une incertitude. Une phrase sur les parents n'est pas un épisode de Victor Hugo, né en 1802.

| Passage | Personne | Source | Statut |
|---|---|---|---|
| Ils se rencontrent à Châteaubriant en 1796 et se marient l'année suivante | Page Victor Hugo | Wikipédia anglaise | Candidat mal attribué : ce sont les parents. Non validé. La même phrase sort deux fois. |
| En 1859, il écrit pour demander la grâce de John Brown | Victor Hugo | Wikipédia anglaise | Candidat. L'encyclopédie cite la lettre ; la lettre elle-même n'a pas été lue. |
| Préceptrice à Varsovie, puis gouvernante à Szczuki chez les Żorawski | Marie Curie | Wikipédia anglaise | Candidat de résidence. La même phrase sort deux fois. |
| Caterina, « usually identified » comme Caterina Buti del Vacca | Léonard | Wikipédia anglaise | Incertain. Le texte dit que l'identification est habituelle, pas établie. |
| Il les aurait rencontrés chez Verrocchio ou à l'Académie platonicienne | Léonard | Wikipédia anglaise | Hypothèse (« would have »). Non validé. |
| La traversée de vingt-cinq jours devient un calvaire | Desbordes-Valmore | Wikipédia française | Candidat de voyage. Non validé. |

Le polonium de juillet 1898 reste une publication déjà stockée. Il n'est plus promu au rang d'anecdote parce que la phrase est longue.

## Chapitres

Les deux lectures déjà prêtes pour une revue restent les campagnes et le pouvoir de Napoléon. Castiglione et Mantoue épaissiraient la campagne déjà classée A. Ils ne créent pas un chapitre, et ils ne font passer aucun B en A ni aucun C en B. Aucun chapitre n'a été écrit.

## Défauts encore ouverts

- La même phrase peut encore sortir deux fois si le radical du lieu diffère.
- Une phrase sur les parents est rattachée au sujet de la page.
- Arcole a été lu et n'a pas produit de bataille retenue.
- Une naissance dont l'année est seulement dans la phrase voisine est maintenant perdue. C'est le prix du refus d'hériter la date du décès.
- Le second essai n'a pas rappelé HAL ni OpenAlex.
- Zéro faux positif observé sur les pièges de V3.7 ne prouve pas leur absence. Les cas adverses couverts par les tests sont Sardaigne, Louvre-Rivoli, « ornée », une notice de catalogue, et l'année du décès collée à la naissance.

## Suite

Chercher le manque, pas le tas de documents. Pour Napoléon, les absents encore utiles sont Arcole, Cherasco et Campo Formio, puis une revue de Castiglione et Mantoue. Pour Hugo, les épisodes hors des 20 000 premiers caractères, sans prendre la vie des parents. Pour Curie, les publications comme documents, avec leur propre niveau de preuve. Ne pas publier un chapitre tant que ses sources et son récit n'ont pas été revus.
