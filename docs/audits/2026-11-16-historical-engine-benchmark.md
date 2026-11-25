# Benchmark du moteur historique — 16 novembre 2026

Mesures en lecture seule sur `talaria_engine_development`. Aucune migration, aucune réingestion, aucune écriture dans cette base. Le correctif de résidence est dans le code ; il ne réécrit pas les lignes déjà stockées.

Référence du code mesuré : `87ac398` (`refactor/talaria-repo-cleanup-v3`). Le classement des verbes de résidence est le correctif de cette branche `bench/historical-engine`.

## 1. Repository et migrations

`feat/ai-supervisors-secure` (`4a0def9`) contient les cinq commits qui ne sont pas sur `origin/feat/ai-supervisors-readonly` (`14547e7`). `refactor/talaria-repo-cleanup-v3` (`87ac398`) ajoute la carte des responsabilités.

Le push direct vers `feat/ai-supervisors-readonly` est refusé par la règle GitHub qui exige une pull request. `gh` n'est pas authentifié dans cet environnement. Les pull requests ne sont donc pas ouvertes.

- Travail fonctionnel : https://github.com/ThibaultL24/TalariaV2/compare/feat/ai-supervisors-readonly...feat/ai-supervisors-secure?expand=1
- Note d'architecture : https://github.com/ThibaultL24/TalariaV2/compare/feat/ai-supervisors-secure...refactor/talaria-repo-cleanup-v3?expand=1

Après `gh auth login` :

```bash
gh pr create --base feat/ai-supervisors-readonly --head feat/ai-supervisors-secure \
  --title "Secure Istoria identity, chapters, and the read-only critic" \
  --body "Five commits already on feat/ai-supervisors-secure. Does not merge to main."

gh pr create --base feat/ai-supervisors-secure --head refactor/talaria-repo-cleanup-v3 \
  --title "Document where Istoria, Poreia, and Agora change" \
  --body "Architecture map only."
```

Migrations appliquées : 1 à 54. `055_editorial_assertions` n'est pas appliquée. `README.md` et `docs/ARCHITECTURE.md` décrivaient 053, 054 et 055 comme un même lot en attente. Le texte suit maintenant l'état lu le 16 novembre 2026.

1 297 événements inactifs n'ont pas d'identité logique. Ils n'ont pas été modifiés.

## 2. Test Versailles

Phrase : « À partir de 1682 la cour demeure au château de Versailles pour tout le règne. »

Le mineur trouvait l'année et Versailles, puis posait le verbe `occurred`, parce que `demeure au` n'était pas dans le lexique. `keywords.rs` retire ensuite le lieu de tout `historical_fact`, pour qu'un fait non typé ne devienne pas un point de carte.

Le comportement attendu est une résidence. `demeure au`, `demeure à`, `demeurait`, `vécut`, `habitait`, `habita au/à`, `séjourna` et `résida` sont classés `lived`, donc `residence`, et le lieu est conservé. Une phrase datée et située sans verbe de résidence reste `occurred`, et son lieu reste retiré.

`year_and_place_without_verb_still_yield_a_point` passe sans changement d'assertion. Le test du judge qui exigeait `occurred` pour cette phrase de résidence a été aligné sur `lived`. Une autre phrase, « la cour se tient au château », couvre encore le cas sans verbe.

`cargo test -p talaria-judge --lib` : 39 réussis. `cargo test -p talaria-sources --test life_trace_extract` : 9 réussis, dont les résidences déjà reconnues par `s'installe` et `dirige son royaume depuis`.

Les résidences stockées de Victor Hugo ne changent pas. Plusieurs sont des énoncés Wikidata `residence · année` dont le lieu est encore un QID (`Q3280244`, `Q898629`). Le lexique ne répare pas ces lignes. Les phrases déjà extraites par `s'installe` (Paris 1812, Marine Terrace à Jersey) restent des résidences.

## 3. Pull requests

Non créées. Liens de comparaison et commandes `gh` à la section 1. Aucun merge vers `main`.

## 4. Corpus initial

19 personnalités ont au moins un événement actif. 3 122 entités existent. Les chiffres détaillés sont dans `2026-11-16-corpus-baseline.json` et `2026-11-16-corpus-by-person.csv`.

| Mesure | Valeur |
|---|---|
| Événements `person` actifs | 2 218 |
| Événements `person` inactifs | 1 299 |
| Identités logiques | 2 218 |
| Chapitres éditoriaux | 0 |
| Candidats assemblés / rejetés / en revue | 3 990 / 2 155 / 93 |
| Ligne de preuve | 47 112 |
| Documents distincts cités par une preuve | 1 124 |
| Documents bruts | 1 355 |

Tous les événements actifs ont une date typée : 2 137 exactes à l'année, 18 exactes au jour, 63 approximatives à l'année. 828 sont éligibles à la carte. 98 n'ont pas de libellé de lieu. 253 n'ont pas de géométrie.

La colonne `date_precision` est vide pour 2 038 lignes. La date sémantique est dans `time_json`, pas dans cette colonne.

`epistemic_status` vaut `attested` pour les 2 218 lignes actives. C'est la valeur par défaut de la colonne. Ce n'est pas une vérification manuelle.

2 198 événements actifs ont un document Wikipédia ou Wikidata. 20 n'en ont pas. 1 635 ont en plus une notice de catalogue.

Napoléon (Q517) : 465 actifs, dont 261 sur la carte. Victor Hugo (Q535) : 208 actifs, 54 sur la carte. Marie Curie (Q7186) : 74 actifs, 19 sur la carte. Léonard de Vinci (Q762) : entité présente, 0 événement. Aristote (Q868) : aucune entité. Aucune personnalité peu documentée n'est dans ce corpus. Le plus petit corpus biographique non politique est Franz Schubert (Q7312), 44 événements actifs.

## 5. Recherche

Pas de nouvel appel aux connecteurs. Une recherche live écrit dans la base, et le budget documentaire d'une personnalité comme Napoléon est celui de l'ingestion complète. Le relevé ci-dessous est le résultat déjà stocké.

| Source | Documents |
|---|---|
| Wikipédia | 392 |
| OpenAlex | 276 |
| HAL | 252 |
| theses.fr | 156 |
| Gallica | 132 |
| Persée | 130 |
| Wikidata | 16 |
| BnF | 1 |

Pour Napoléon, les preuves actives citent 109 pages Wikipédia, 27 notices HAL, 24 OpenAlex, 20 theses.fr, 16 Persée, 11 Gallica, 1 Wikidata. Hugo n'a que 3 pages Wikipédia liées, contre 25 HAL et 20 theses.fr. Curie a 3 pages Wikipédia et 12 Gallica.

Un document de catalogue n'est pas, ici, une source nouvelle d'un fait. Pour les 190 batailles actives de Napoléon : 5 161 lignes de preuve, 289 citations distinctes. Wikipédia fournit 191 lignes et 191 citations, soit environ une citation par bataille. OpenAlex fournit 3 060 lignes pour 24 citations distinctes. La même phrase (« Battle of Lodi was fought on Tuesday 10 May 1796… ») est rattachée à des titres de livres (`Germany from Napoleon to Bismarck`, `Naples and Napoleon`). Le document est hors sujet en tant que preuve directe. La citation, elle, décrit bien la bataille.

Le jeu reproductible côté extraction est `crates/talaria-sources/tests/life_trace_extract.rs` : 9 phrases attendues, 9 réussites après le correctif. Ce n'est pas un rappel mesuré sur un échantillon annoté de Napoléon.

Coût IA de cette campagne : 0. Aucun appel de modèle.

## 6. Ingestion

Le pipeline `resolve → collect → extract → ground → gate → persist` n'a pas été relancé. Il n'existe pas, dans cette session, de base éphémère chargée avec un budget de documents. Les comptes suivants décrivent l'ingestion déjà persistée.

| Issue | Actifs |
|---|---|
| Candidats assemblés | 3 990 |
| Rejetés | 2 155 |
| En revue | 93 |
| Acceptés encore en table candidate | 1 |
| Événements canoniques actifs | 2 218 |
| Dont `historical_fact` | 1 175 |
| Dates exactes (année ou jour) | 2 155 |
| Dates approximatives conservées comme `approx` | 63 |

Les 190 batailles de Napoléon ont toutes une pièce Wikipédia. Les notices HAL, OpenAlex, Gallica et theses.fr ne sont pas leur seule preuve. Elles sont attachées en plus, souvent avec la citation Wikipédia recopiée. Elles ne sont donc pas devenues le fait, mais elles gonflent le nombre de preuves.

Aucune précision ni aucun rappel n'est calculé sur un jeu annoté à la main au-delà des 9 phrases du test de life-trace.

## 7. Qualité des preuves

Une ligne de preuve n'est pas un passage distinct. 47 112 lignes couvrent 1 124 documents et 3 497 événements, actifs et inactifs confondus.

Pour les batailles de Napoléon, le passage exploitable est la citation Wikipédia (191 textes pour 190 événements). Les milliers de lignes de catalogue répétent une petite poignée de ces textes sous un autre titre. Elles ne doivent pas être comptées comme des preuves indépendantes.

`attested` ne signifie pas qu'une preuve directe a été vérifiée par une personne.

## 8. Anecdotes

Définition utilisée, sans écrire de nouveau type : un fait singulier, secondaire, utile pour comprendre la personne, et porté par une citation identifiable. Le type canonique `anecdote` n'est qu'un signal, pas cette définition.

Cinq lignes actives ont le type `anecdote`. Deux disent elles-mêmes qu'elles ne sont pas établies :

- Hugo : « An apocryphal tale… the shortest correspondence in history ».
- Molière : « Louis XIV invites Molière to share his supper—an unfounded Romantic anecdote ».

Les trois autres sont une nouvelle d'atelier apprise par Van Gogh, la mort de la fille de Hugo apprise pendant un voyage, et une phrase générique sur l'« épopée » de Jeanne d'Arc. Aucune de ces cinq lignes n'est retenue ici comme anecdote documentée prête à raconter. Les 1 175 `historical_fact` contiennent sans doute des épisodes, mais ils n'ont pas été lus un par un. Les compter serait inventer un chiffre.

Anecdotes documentées retenues : 0. Anecdotes candidates de type `anecdote` : 5, dont 2 explicitement apocryphes.

## 9. Chapitres

Aucun chapitre n'est publié : `editorial_groups` est vide. Les propositions ne sont pas insérées.

Règle, appliquée aux seuls types déjà posés, pas au tas `historical_fact` :

- A — au moins 15 événements actifs du même thème, chacun avec un document Wikipédia ou Wikidata.
- B — de 6 à 14 événements dans ce cas, ou un thème plus grand dont les lieux sont encore des QID ou des fragments.
- C — de 1 à 5 événements, thème reconnaissable, corpus trop mince.
- D — regrouper les `historical_fact` sous un titre de chapitre.

| Personne | Proposition | Classe | Événements typés | Lacune |
|---|---|---|---|---|
| Napoléon | Campagnes | A | 190 batailles, 2 sièges | les catalogues répètent les mêmes citations |
| Napoléon | Pouvoir et diplomatie | A | 17 offices, 15 diplomatiques | — |
| Napoléon | Déplacements | B | 10 résidences, 4 voyages, 4 exils, 15 arrivées, 5 départs | arrivées et résidences se recouvrent |
| Napoléon | Famille | B | 9 mariages, naissance, mort | — |
| Hugo | Résidences | B | 19 | plusieurs lieux sont des QID ou un bout de phrase |
| Hugo | Œuvres | B | 14 publications | seulement 3 pages Wikipédia liées à l'ensemble du dossier |
| Hugo | Mandats | B | 11 offices | — |
| Hugo | Anecdotes | C | 2 | les textes se disent apocryphes ou indirects |
| Curie | Études | B | 6 | — |
| Curie | Découverte et publications | C | 2 publications, dont le polonium en juillet 1898 | pas de série de faits de laboratoire |
| Curie | Scandale de 1911 | C | 1 | un seul épisode |
| Léonard | — | — | 0 | entité sans événement |
| Aristote | — | — | 0 | personne absente |
| Personnalité peu connue | — | — | 0 | le corpus n'en contient pas |

Total des propositions : 2 A, 7 B, 3 C, 0 D rédigées. Le tas `historical_fact` (168 pour Napoléon, 118 pour Hugo, 46 pour Curie) est laissé en D implicite : ce n'est pas un chapitre.

Schubert, hors de la liste demandée, donnerait un B « Œuvres » (18 publications). Ce n'est pas une personnalité obscure.

## 10. Avant et après

Aucune ingestion nouvelle n'a été exécutée. Le gain mesuré est donc nul.

| | Déjà présent | Nouveau validé | Nouvelle anecdote retenue | Nouveau chapitre publié |
|---|---|---|---|---|
| Napoléon | 465 | 0 | 0 | 0 |
| Hugo | 208 | 0 | 0 | 0 |
| Curie | 74 | 0 | 0 | 0 |
| Léonard | 0 | 0 | 0 | 0 |
| Aristote | 0 | 0 | 0 | 0 |

Le correctif de résidence pourra, lors d'une prochaine extraction, garder le lieu des phrases `demeure au` et `vécut`. Il n'a pas produit de nouvel événement dans cette base.

## 11. Performance et coût

Les requêtes de lecture ont pris moins d'une seconde chacune, sauf le décompte des preuves longues (environ 4 secondes). Les tests Rust du correctif ont pris une compilation d'une dizaine de secondes, puis les assertions en moins d'une seconde. Coût de modèle : 0. Aucun connecteur live n'a été chronométré, parce qu'il n'a pas été appelé.

## 12. Lacunes

- La moitié des faits actifs sont des `historical_fact` (1 175 sur 2 218). Les chapitres ne peuvent pas s'appuyer dessus sans une relecture.
- Les catalogues multiplient les lignes de preuve sans multiplier les passages (3 060 lignes OpenAlex, 24 citations, 190 batailles).
- 19 biographies seulement. Léonard est résolu et vide. Aristote est absent. Aucune personnalité peu connue.
- Des lieux de résidence restent des QID ou des fragments (« même temps que Victor Hugo »).
- `attested` est affiché pour tout le corpus actif alors que personne ne l'a vérifié ligne à ligne.
- `date_precision` est vide alors que `time_json` est rempli.
- Le type `anecdote` ramasse des récits que le texte lui-même dit apocryphes.
- Zéro chapitre éditorial, et la migration 055 qui porterait les phrases de chapitre n'est pas appliquée.

## 13. Cinq corrections, par rendement

1. N'attacher une notice HAL, OpenAlex, Gallica, Persée, theses.fr ou BnF que si la citation est un passage de ce document. Aujourd'hui la citation Wikipédia est recopiée sous des titres de livres. Correctif déterministe à l'écriture de `event_evidence`. C'est le plus gros écart entre volume affiché et preuve réelle.
2. Continuer le lexique de verbes pour sortir les faits du type `historical_fact`, comme pour `demeure au`. 1 175 lignes sont concernées. Pas de nouvel agent.
3. Résoudre les libellés de lieu qui sont encore des QID ou des bouts de phrase, avec le gazetteer déjà utilisé à l'ingestion. Les résidences de Hugo montrent le trou.
4. Cesser de présenter le défaut `attested` comme une attestation. Le statut doit rester vide ou `uncertain` tant qu'une règle ne l'a pas posé.
5. Lancer, sur une base éphémère et avec un plafond de documents, l'ingestion des entités déjà créées mais vides, en commençant par Léonard (Q762). Aristote (Q868) n'a même pas d'entité. Cela mesure enfin la généralisation, que ce corpus ne permet pas.

## Réponses courtes

Talaria a 2 218 événements actifs de pipeline `person`, et 1 299 versions inactives. 2 198 actifs ont un document Wikipédia ou Wikidata. 828 peuvent aller sur la carte. 63 dates sont approximatives et le restent.

Les anecdotes prêtes à être racontées, au sens d'un épisode singulier et non démenti par sa propre citation, sont 0 dans le type `anecdote`. Le reste n'a pas été promu à la main.

Deux chapitres sont prêts pour une revue éditoriale (les campagnes et le pouvoir de Napoléon). Sept sont des brouillons crédibles. Trois sont des pistes trop minces. Rien n'a été publié.

Cette campagne n'a découvert aucun fait nouveau : elle n'a pas relancé la recherche. Le gain disponible le plus net est de ne plus compter une notice bibliographique comme une preuve de plus.
