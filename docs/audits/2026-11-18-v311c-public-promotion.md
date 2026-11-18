# V3.11c — La publication publique ne suit plus l'acceptation technique

Date : 18 novembre 2026. Branche `feat/v311c-public-promotion`, issue de `feat/v311b-end-to-end-biography` (`327f230`).

La base `talaria_engine_development` n'a pas été écrite. La migration 055 n'y a pas été appliquée. Les 38 événements ci-dessous viennent de la base éphémère `talaria_v311b_ephemeral`, relue sans modification. Aucun fait canonique existant n'a été réécrit.

## 1. Les 38 événements admis pendant V3.11b

Personnalité : Marceline Desbordes-Valmore, Q466987. Les passages sont ceux stockés dans `event_evidence`. « Année dans la phrase » veut dire que les quatre chiffres figurent dans le passage, pas dans un autre.

| Id | Type | Passage | Année stockée | Année dans la phrase | Lieu stocké | Carte | Classe |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `37a64abf` | travel | the young girl somehow made her way back to France | approx 1786 | non | France | oui | date non justifiée, lieu non justifié |
| `a93d4256` | birth | fiche Wikidata `born_in 1786` | exact 1786 | oui, énoncé structuré | Douai | oui | correct, imprécis (année seule) |
| `42ac6042` | travel | traversée, puis « en mai 1802, la mère » | exact 1802 | oui, mais pour la mort de la mère | — | non | incertain |
| `d6186cc8` | work | devient comédienne à Douai | approx 1802 | non | Douai | oui | date non justifiée |
| `eec08616` | travel | Marceline part rejoindre son père | approx 1802 | non | — | non | date non justifiée |
| `9784a3f8` | publication | Le Billet, publiée en 1807 | exact 1807 | oui | — | non | correct |
| `ad6d391e` | work | la même phrase, sans le point-virgule | exact 1807 | oui | — | non | doublon |
| `e8f20708` | departure | quitte son amant, Odéon puis Bruxelles | approx 1808 | non | Bruxelles | oui | date non justifiée |
| `bffadd85` | work | et à Paris | approx 1815 | non | Paris | oui | passage trop mince |
| `533cb9ea` | work | Sa fille Hyacinthe compose des poèmes | approx 1816 | non | — | non | attribution erronée |
| `01ee8ac4` | other | se marie en 1817, rencontré à Bruxelles | exact 1817 | oui | Bruxelles | non | correct, lieu imprécis |
| `3fc727a0` | marriage | fiche Wikidata `married 1817` | exact 1817 | oui, énoncé structuré | Q33838374 | non | correct, imprécis |
| `0d2166ef` | work | appeared in Douai, Rouen, Paris, Brussels | approx 1817 | non | Douai | oui | date non justifiée |
| `41089087` | work | la même phrase | approx 1817 | non | Rouen | oui | date non justifiée |
| `7dfef67e` | work | la même phrase | approx 1817 | non | Brussels | oui | date non justifiée |
| `b194fbce` | work | la même phrase | approx 1817 | non | Paris | oui | date non justifiée |
| `2bf08ce7` | work | published Élégies et Romances in 1819 | exact 1819 | oui | — | non | correct |
| `5c4e7a9c` | publication | la même phrase | exact 1819 | oui | — | non | doublon |
| `5b56386f` | residence | Le couple s'installe ensuite à Lyon. | approx 1819 | non | Lyon | oui | date non justifiée, doublon |
| `cddfc1d4` | residence | la même phrase, sans le point | approx 1819 | non | Lyon | oui | date non justifiée, doublon |
| `9b5eadf8` | work | En 1820 paraissent les Poésies | exact 1820 | oui | — | non | correct |
| `5504c565` | work | a mis en musique L'Alouette | approx 1821 | non | — | non | date non justifiée |
| `f8906fe3` | historical_fact | la phrase de carrière anglaise, plus longue | exact 1821 | non | — | non | date non justifiée |
| `2b567229` | office | Après 1823, quitte le théâtre | exact 1823 | oui, borne ouverte | — | non | doublon, précision abusive |
| `981d76d9` | office | la même phrase | exact 1823 | oui, borne ouverte | L'écriture | non | lieu non justifié |
| `ac6ca459` | work | la même phrase, avec un point | exact 1823 | oui, borne ouverte | — | non | doublon |
| `bd62977e` | departure | la même phrase | exact 1823 | oui, borne ouverte | écriture | non | lieu non justifié |
| `3d7b0e29` | work | romances composées par Louise de Caumont | approx 1841 | non | — | non | attribution erronée |
| `5d637a8d` | meeting | Henri Woollett compose la suite | exact 1841 | non | — | non | attribution erronée |
| `090c2a5f` | trial | intervient dans le procès le 30 mai 1845 | exact 1845-05-30 | oui | — | non | correct, type imprécis |
| `b4e1fd02` | other | la même phrase | exact 1845-05-30 | oui | — | non | doublon |
| `4f28dab5` | historical_fact | est la fille de Catherine Lucas | exact 1859 | non | — | non | date non justifiée |
| `a3fcf23e` | death | fiche Wikidata `died_in 1859` | exact 1859 | oui, énoncé structuré | Paris | oui | correct, imprécis |
| `889cda79` | historical_fact | meurt à Paris le 23 juillet 1859 | exact 1859-07-23 | oui | — | non | correct |
| `8a1707ea` | commemoration | deuxième édition sur Gallica, Genève, 1873 | exact 1873 | oui | Genève | non | notice éditoriale |
| `19ddc54d` | statue | Édouard Houssin, monument, 1896 | exact 1896 | oui | — | non | notice, autre acteur |
| `24f2ca28` | commemoration | Ouvrage collectif, Imprimerie Crépin, Douai, 1896 | exact 1896 | oui | Douai | non | notice éditoriale |
| `492d6cd5` | commemoration | Auguste Bleton, Impr. de A. Rey, Lyon, 1896 | exact 1896 | oui | Lyon | non | notice éditoriale |

## 2. Erreurs certaines

38 lignes. 29 ne devaient pas être publiques dans l'état stocké. 8 sont acceptables, parfois avec une année seule ou un type trop large. 1 est incertaine : le voyage de Guadeloupe, dont le « mai 1802 » date la mort de la mère dans le même passage.

Taux d'erreurs certaines : 29/38. Cas incertains : 1/38. Le reste, 8/38, peut rester, à condition de ne pas présenter l'année seule comme un jour.

Causes, dans le code d'alors :

- La page biographique recevait `TitleSubjectMatch` avant de regarder qui fait l'action. Hyacinthe, Louise et Woollett passaient.
- Une année voisine, souvent l'année de naissance, datait une phrase qui n'en contient pas. D'où 1786 sur le retour en France.
- « Après 1823 » dans une phrase longue devenait une date exacte. « écriture » devenait un lieu.
- Un pays résolu par le gazetteer recevait un point.
- Une notice d'édition n'était reconnue que si elle commençait par une étiquette de catalogue. « sur Gallica » et « Impr. » passaient, et les types commémoration ou statue échappaient au contrôle de décès.
- Deux formulations de la même phrase, ou deux types extraits de la même phrase, produisaient deux clés d'occurrence.

## 3. Modifications du moteur

Pas de nouvel agent. Quatre contrôles s'ajoutent au chemin qui existait déjà.

- `admit_to_public_corpus` refuse la publication si l'acteur est un proche ou un tiers, si le passage est une notice, une filiation seule, ou un fragment, ou si l'année vient d'une autre phrase. Le candidat reste `needs_review`.
- Une année de naissance dans une autre phrase ne date plus le fait. « à l'âge de seize ans », un titre de section, et l'année d'inhumation restent des rattachements explicites. « Après 1823 » est une date approximative.
- Un pays ne reçoit pas de coordonnées. Deux villes dans la même phrase ne deviennent pas un seul point. Un identifiant Wikidata nu n'est plus un nom de lieu.
- La même phrase, au point final près, renforce l'événement déjà admis au lieu d'en créer un second. Une phrase de décès rejoint le décès déjà admis.

## 4. Tests de non-régression

`cargo test -p talaria-quality --lib` : 153 tests passés.

Le test éphémère `doubtful_lines_do_not_enter_the_public_biography` rejoue les phrases obligatoires. Hyacinthe et Gallica ne deviennent pas canoniques. Aucune ligne publique n'a 1786, la France n'est pas sur la carte, « écriture » n'est pas un lieu, et les Poésies de 1820 restent datées de 1820.

`reviewed_italian_events_persist_once_on_a_fresh_database` passe encore : Arcole, Castiglione, Mantoue et Cherasco restent publics ; Dego, Campo Formio, Ceva et Tolentino restent en revue. Mantoue garde son intervalle de jours.

Les tests unitaires couvrent aussi Victor Hugo à Guernesey, la noyade de Léopoldine, une réédition sur Gallica, l'annonce du polonium par Marie Curie, une réimpression de sa thèse, et la naissance d'Aristote en 384 avant l'ère commune, sans jour inventé.

## 5. Contrat de promotion

Quatre états, dans l'ordre :

1. Extraction réussie : le passage est conservé.
2. `Accept` technique : les portes de qualité et l'attribution automatique passent.
3. Admissibilité documentaire : l'acteur, la date, le lieu et la nature du document tiennent dans le passage.
4. Publication : seulement si 2 et 3 passent.

Sinon le candidat reste en revue, avec le motif dans `judgment_json.public_admission`. Une recherche terminée n'est pas une biographie vérifiée. L'interface le dit, et elle ne dit jamais « Biographie vérifiée ». Les suggestions de chapitres restent des suggestions.

Si une preuve déjà citée devient invalide, le mécanisme d'assertion existant la marque `needs_review` sans réécrire le titre du fait. Rien de nouveau n'a été branché là-dessus, et 055 n'est toujours pas sur la base de développement.

## 6. Avant / après, mêmes passages

Il n'y a pas eu de second moissonnage de Wikipédia. Les mêmes phrases ont été soumises au nouveau contrat.

| Passage | Avant | Après |
| --- | --- | --- |
| Retour en France | public, 1786, point sur la France | pas de date héritée, pas de point |
| Hyacinthe | public, œuvre de Marceline | revue, acteur |
| Gallica 1873, statue, deux notices de 1896 | publics | revue, notice |
| Lyon, deux fois, 1819 | deux faits datés | une phrase, sans année héritée |
| Poésies, 1820 | public | public, 1820 |
| Après 1823 / écriture | quatre faits, deux lieux fictifs | une borne ouverte, sans ce lieu |
| Arcole, Castiglione, Mantoue, Cherasco | publics | publics |

Ce qui était vrai peut rester public sans date quand la phrase n'en porte pas : rejoindre son père, mettre L'Alouette en musique, s'installer à Lyon. Ce qui était attribué au mauvais acteur, ou copié sur une notice, ne l'est plus.

## 7. Ce qui resterait admis

Resteraient admissibles, sur ces passages : la naissance et la mort structurées, Le Billet, les Élégies, les Poésies de 1820, le mariage de 1817, l'intervention du 30 mai 1845, et le décès du 23 juillet 1859. Le départ du théâtre après 1823 peut rester, comme borne ouverte et sans lieu « écriture ».

Restent en revue : Hyacinthe, Louise, Woollett, les quatre notices, le fragment « et à Paris », et la phrase de filiation datée à tort de 1859.

Le voyage de 1802 reste le point faible : l'année est dans le passage, mais elle date la mort de la mère. Le contrat ne sépare pas encore deux faits à l'intérieur d'un seul paragraphe long.

## 8. Protections contre les abus

Elles s'ajoutent au plafond de huit recherches simultanées et à la déduplication d'une identité déjà en cours.

- Trois recherches par identité et par heure. Au-delà : HTTP 429 `ingest_quota`, message explicite.
- Quatre nouvelles recherches par minute, toutes identités confondues.
- Une recherche déclenchée avec au plus huit documents limite aussi les appels de corpus : quatre par source, seize au total.
- Quatre appels de modèle au plus par page. Les règles lisent encore toute la page.
- Les tentatives restent bornées à trois, avec un bail de 90 secondes.

Il n'y a pas encore de compte visiteur. La limite par identité est la protection contre une recherche relancée en boucle. Une limite par adresse demanderait une identité authentifiée, qui n'existe pas sur Istoria.

## 9. Navigateur

Le texte affiché sur une fiche distingue une recherche terminée d'une biographie vérifiée. Les suggestions disent qu'elles ne sont pas un récit confirmé. Aucun pourcentage n'est ajouté.

Le parcours Playwright a été rejoué contre l'API branchée sur la base éphémère, sans nouvelle ingestion : 5 tests passés, le test mobile de recherche sauté comme auparavant. Ordinateur et mobile couvrent le nom, l'absence de fiche, et, sur ordinateur, la fiche de Desbordes-Valmore avec la frise, la carte, le rechargement, et la phrase qui refuse d'appeler cela une vérification. Les 38 lignes anciennes sont encore dans cette base : ce lot ne les réécrit pas. Un parcours vert ne corrige pas leur contenu.

## 10. Activation sur la base de développement

Non faite. Elle demanderait, dans un lot séparé :

1. Une autorisation explicite pour appliquer la migration 055.
2. Une sauvegarde de `talaria_engine_development` avant cette migration.
3. Aucune republication des faits canoniques déjà admis.
4. La recherche universelle encore limitée aux personnalités choisies pour l'essai, avec le contrat de la section 5.
5. Une lecture humaine des premières fiches avant d'élargir.

Tant que ce lot n'est pas décidé, `PUBLIC_PERSON_SEARCH_ENABLED` reste faux hors d'Istoria, et Istoria ne doit pas être ouvert sur la base de développement.
