# V3.9 — recherche ciblée des faits manquants

Mesure du 16 novembre 2026, branche `feat/v39-targeted-recovery`, partie de `feat/v38-historical-dedup` (`121bb7e`). Aucune écriture dans `talaria_engine_development`. Migration 055 non appliquée. Aucun fait canonique modifié, aucune fusion, aucun chapitre publié. Coût : 0.

Le benchmark à conditions égales est l'ensemble des tests déterministes. L'essai live est séparé : huit pages d'événements, puis les biographies. Les fichiers V3.7 et V3.8 n'ont pas été réécrits.

## Où l'information disparaissait

| Événement | Découverte | Passage | Étape qui bloquait en V3.8 | V3.9 |
|---|---|---|---|---|
| Arcole, 15–17 novembre 1796 | La page est lue, 23 189 caractères. Le nom est au caractère 4. | « The Battle of Arcole … was fought … Napoleon Bonaparte's French Army of Italy. » | La phrase du combat ne contient pas le nom. Le filtre lisait « Battle » comme une personne. | Extrait. Année 1796, lieu Arcole. Distinct de Lonato. Non écrit. |
| Cherasco, 28 avril 1796 | Page entière, 1 029 caractères. | « The Armistice of Cherasco was a truce signed at Cherasco … and Napoleon Bonaparte. » | « Armistice » n'était pas une action reconnue. | Accord diplomatique, lieu Cherasco, 1796. Non écrit. |
| Campo Formio, 17 octobre 1797 | Page de 5 888 caractères. Le traité est dans l'ouverture. | « The Treaty of Campo Formio was a peace treaty signed on 17 October 1797… » | Le titre seul n'est pas une preuve, et « treaty » ne comptait pas comme un récit. | Traité, lieu Campo Formio, 1797. Non écrit. |
| Castiglione, 5 août 1796 | Ouverture. | « The Battle of Castiglione saw the French Army of Italy under General Napoleon Bonaparte attack … on 5 August 1796. » | Déjà extrait en V3.8. | Passage confirmé. Lieu Castiglione. Distinct. Non écrit. |
| Mantoue | Ouverture. | « The siege of Mantua lasted from 4 June 1796 to 2 February 1797 … Napoleon Bonaparte. » | Déjà extrait en V3.8. | Siège, lieu Mantua. Les deux dates sont dans la phrase. L'année retenue est 1796, la première. Non écrit. |
| Dego, 14–15 avril 1796 | « Battle of Dego » ne renvoie qu'une page trop courte. | « The Second Battle of Dego was fought on 14 and 15 April 1796 … near Dego. » | La page alias n'était pas demandée. | Extrait depuis « Second Battle of Dego ». Non écrit. |
| Ceva, 16 avril 1796 | Ouverture. | « In the Battle of Ceva on 16 April 1796 … under General Pierre Augereau fought… » | Non cherché en V3.8. | Extrait, lieu Ceva. La phrase d'action nomme Augereau. L'attribution à Napoléon reste à revoir. Non écrit. |
| Tolentino, 2–3 mai 1815 | Ouverture. | « The Battle of Tolentino was fought … by the Napoleonic King of Naples Joachim Murat. » | Non cherché. | La page est lue. L'adjectif « Napoleonic » n'est pas le nom Napoléon. Le combat est celui de Murat. Ce n'est pas un fait nouveau de Napoléon. |

Arcole n'était pas hors des 4 000 premiers caractères. Ce n'était pas un doublon de Lonato. C'était un refus d'attribution, sur une phrase qui décrit le combat et sur la phrase suivante qui nomme Bonaparte.

## Identité et rapprochement

`type|année|radical du lieu` reste une heuristique de regroupement pour une passe d'extraction. L'identité historique est `occurrence_key` : personne, type, action, date, lieu, objet. Deux rencontres à Paris en 1796, l'une avec Joséphine et l'autre avec Barras, ne forment pas un seul fait. Trois extracteurs sur la même phrase de Montenotte n'en forment qu'un.

Avant de dire qu'un fait est nouveau, le rapprochement donne :

- un fait existant, comme Lonato 1796 dont le lieu stocké est « Italy » ;
- une preuve supplémentaire, quand une autre phrase parle du même combat ;
- un fait distinct, comme Arcole, Castiglione ou Dego, non écrit ;
- un rapprochement incertain, comme Bassano sans année, ou Ceva tant que la phrase de combat nomme un autre général.

Aucune identité logique n'est fusionnée.

## Anecdotes

Aucune n'est approuvée. Une page Wikipédia ne suffit pas.

| Piste | Passage | Niveau |
|---|---|---|
| Lettre pour John Brown | « a letter asking the United States government … to spare abolitionist John Brown's life ». Caractère 16 636 de la page anglaise de Hugo. La phrase « In 1859, he wrote a letter » avait déjà été lue en V3.8. | Documentée, non approuvée. |
| Exil à Jersey | « He moved to Brussels, then Jersey, from which he was expelled… », après le coup de 1851. Caractère 13 713. | Documentée, non approuvée. |
| Gouvernante à Szczuki | « then for two years as a governess in Szczuki with a landed family, the Żorawskis ». Caractère 5 681 de la page de Marie Curie. La phrase n'a pas d'année. | Candidate. |
| Naissance de Desbordes-Valmore | « née le 20 juin 1786 à Douai et morte le 23 juillet 1859 à Paris ». Page française. L'ouverture anglaise de V3.7 ne donnait pas cette date. | Documentée, non approuvée. |
| Traversée devenue un calvaire | Vue dans la page française en V3.8. | Candidate. |

« Usually identified », pour la mère de Léonard, reste une candidate. Le mot « Napoleonic » ne fait pas de Tolentino un événement de Napoléon.

## Chapitres

Les deux lectures déjà prêtes pour une revue restent les campagnes et le pouvoir. Arcole, Castiglione, Dego et le siège de Mantoue épaissiraient la campagne, après revue. Cherasco et Campo Formio épaissiraient le chapitre diplomatique, déjà classé A. Ceva et Tolentino n'ajoutent pas un chapitre. Aucun classement ne change. Aucun chapitre n'est créé.

## Comparaison

| Indicateur | V3.8 | V3.9 |
|---|---|---|
| Arcole | Page lue, aucun événement | 1796, Arcole, passage conservé, non écrit |
| Faits distincts proposés | Castiglione et Mantoue | Arcole, Castiglione, Dego, Mantoue, Cherasco, Campo Formio |
| Faits écartés après lecture | — | Tolentino (Murat), Ceva (Augereau dans la phrase d'action) |
| Anecdotes approuvées | 0 | 0 |
| Passages documentés, non approuvés | 0 | Lettre Brown, Jersey, naissance à Douai |
| Chapitres prêts pour revue | 2 | 2 |
| Durée de l'essai ciblé | 18 s pour six biographies | 34 s pour huit pages d'événements |

La baisse du nombre de propositions n'est pas le gain. Le gain est de récupérer le passage d'Arcole, de Cherasco et de Campo Formio, et de ne pas compter Tolentino.

## Cinq corrections suivantes

1. Garder les deux dates d'une même phrase. Mantoue commence en juin 1796 et se rend le 2 février 1797.
2. N'attribuer le combat qu'à la personne nommée dans la phrase d'action. Augereau à Ceva, Murat à Tolentino.
3. Demander l'alias quand le titre court est vide. « Battle of Dego » l'était ; « Second Battle of Dego » portait le passage.
4. Séparer la lecture des biographies des rafales de pages d'événements. Hugo et Curie ont été relus après un refus 429.
5. Ne pas écrire ces candidats dans la base réelle avant une revue. V3.10 peut le faire. V3.11 peut en faire des chapitres.
