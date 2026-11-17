# V3.11b — parcours réel d'une biographie à la demande

Date : 18 novembre 2026. Branche `feat/v311b-end-to-end-biography`, issue de `feat/v311-on-demand-biography` (`386d97f`).

La base `talaria_engine_development` est restée à la migration 54, avec 3 517 événements canoniques. La migration `055` n'y a pas été appliquée. Aucun chapitre n'a été publié. Le serveur a refusé de démarrer sur cette base : `refusing to apply pending migrations [55] without TALARIA_APPLY_MIGRATIONS=1`.

## 1. Parcours réellement fonctionnel

Sur une base sœur, `talaria_v311b_ephemeral`, avec toutes les migrations y compris `055`, l'API a démarré sans `TALARIA_APPLY_MIGRATIONS`. Le LLM était coupé. Istoria, dont la recherche publique était fermée (`PUBLIC_PERSON_SEARCH_ENABLED` reste faux pour l'explorateur), ouvre maintenant sa propre recherche.

Playwright a conduit Chromium du système, en bureau et en mobile. Le Chromium téléchargé par Playwright ne s'installe pas sur Ubuntu 20.04. Cinq tests ont réussi, le sixième est le même parcours de recherche limité au bureau.

Ce qui a été fait dans le navigateur :

1. Saisie de « Dumas », sans QID. Alexandre Dumas (Q38337) et Alexandre Dumas fils (Q169150) sont proposés, distincts. Un homonyme exact, « Dumas, French official » (Q124788338), apparaît aussi. Rien n'est fusionné. Choisir le père affiche « Identifiant retenu : Q38337 » et le bouton « Lancer la recherche », encore activable. Aucune fiche n'est créée par ce choix.
2. Saisie de « zzzznotapersonqqq ». Le seul choix n'a pas de QID. Le texte dit qu'aucune fiche n'est créée. Il n'y a pas de bouton de recherche.
3. Saisie de « Marceline Desbordes-Valmore », choix de Q466987, clic sur « Lancer la recherche ». Le job durable est parti. La progression affiche la phase et le nombre d'événements, pas un pourcentage. À la fin, la fiche s'ouvre. La frise Pixi.js et la carte MapLibre se montent. Recharger la page retrouve la fiche et propose « Enrichir le corpus », sans relancer la recherche.

« Aristote » en français et « Aristotle » en anglais résolvent Q868, philosophe, devant les homonymes (université, genre biologique, prénom). Le premier appel anglais à froid est revenu vide : le délai Wikidata de 900 ms était trop court. Il est passé à 2,5 s. Les appels suivants ont rendu Q868 en une demi-seconde.

## 2. Ce qui reste une épreuve, pas une biographie vérifiée

Les six « publications » d'Aristote de la V3.11 n'étaient pas des sources. Le test écrivait, pour chaque année de 340 à 335 avant notre ère : « Aristotle published a treatise in Athens in N BC. » Le mot « published », Athènes et les six années venaient de la boucle, pas d'un document. Rien ne distinguait composition, enseignement, transmission et publication. Le parseur en faisait des années exactes. Ces lignes ont été retirées. Une naissance seule ne propose plus de chapitre. Le seuil « six faits d'un type » reste un test de compte, sans nom de personne.

La recherche navigateur, elle, a lu de vraies pages, sans LLM. Le job s'est terminé `done`. Ce statut veut dire que le pipeline a fini, pas que les faits sont établis.

Résultat sur la base éphémère, Q466987 :

| Issue | Nombre |
|---|---|
| Événements canoniques actifs | 38 |
| Dont placés sur la carte | 12 |
| Candidats `needs_review` | 41 |
| Candidats rejetés | 20 |
| Chapitres `published` | 0 |

Phrases retenues et jugement :

- Naissance : `Marceline Desbordes-Valmore | birth | born_in | 1786 |`. C'est une fiche Wikidata sérialisée, pas un passage. L'année 1786 est la bonne tradition, le jour du 20 juin n'a pas été inventé. Douai est le lieu Wikidata. La carte peut montrer la localité comme repère, pas une adresse.
- Mort : la fiche Wikidata dit 1859 à Paris, en année. Une autre ligne porte le jour `1859-07-23`. Le jour existe dans les sources usuelles. Il ne faut pas le mélanger avec la fiche qui, elle, ne le donne pas.
- « En 1820 paraissent les Poésies de Mme Desbordes-Valmore. » L'année est dans la phrase. C'est une parution. Pas de lieu. C'est le fait le plus solide du lot.
- « Marceline part rejoindre son père après le décès de sa mère. » La phrase n'a pas d'année. L'événement a reçu 1802. L'année est inférée.
- « Le couple s'installe ensuite à Lyon. » Pas d'année dans la phrase, deux lignes canoniques pour la même installation, année approximative 1819. Lyon est dans la phrase. Le dédoublon n'a pas tenu.
- « Après 1823, Marceline Desbordes-Valmore quitte définitivement le théâtre pour se consacrer à l'écriture. » « Après 1823 » est devenu une année exacte. « écriture » et « L'écriture » sont devenues des lieux, et l'une une fonction. Elles ne sont pas sur la carte. Le mot n'est plus accepté comme lieu pour les prochaines extractions.
- « compose des poèmes et des contes », titre « Sa fille Hyacinthe… ». Le fragment concerne la fille. Il est quand même sur la fiche de Marceline, parce que la page biographique suffit aujourd'hui à l'attribution.
- « the young girl somehow made her way back to France. » Phrase anglaise, au conditionnel du récit, sans année. Elle est devenue un voyage vers la France, sur la carte, vers 1786. Un pays nommé dans une phrase ne devait pas devenir le lieu de l'événement. C'est le défaut le plus net de cette course.
- « deuxième édition (Poésies de Mme Desbordes-Valmore publiées par Gustave Revilliod) sur Gallica, Genève, Jules Fick, 1873. » Notice d'édition, quatorze ans après la mort. Elle est devenue une commémoration en 1873. Genève n'a pas été mise sur la carte. La ligne publique reste fausse.
- « Édouard Houssin, Monument à Marceline Desbordes-Valmore, 1896… » Monument posthume, pas un acte de sa vie. Le type `statue` est dispensé du contrôle « après la mort », parce qu'une commémoration peut être posthume. Ici la dispense a laissé passer une notice de monument.
- Mariage : `married | 1817 | Q33838374`. L'identifiant de lieu brut a été gardé comme libellé, hors carte.

Les 41 lignes en revue et les 20 rejets ne sont pas sur la frise ni sur la carte. Les 38 lignes canoniques, elles, sont montrées comme des faits. Plusieurs ne devraient pas l'être. Un `Accept` technique plus une page dont le titre est la personne ne suffit pas.

## 3. Politique de publication

Une ligne n'entre dans le corpus public que si `publishes_to_public_corpus` est vrai : décision `Accept`, et attribution qui peut tenir seule (nom direct, alias, page de la personne, ou action militaire suivie avec un rôle démontré). `NeedsReview` et `Reject` restent des candidats.

Ce n'est pas une validation éditoriale. Avant une ouverture publique, une page biographique devrait en plus exiger que l'année soit dans la phrase citée, et qu'un lieu, s'il est gardé, soit un lieu plutôt qu'un nom commun ou un pays seul. Cette course ne remplit pas cette seconde exigence. Elle ne doit pas servir de corpus de démonstration sans relecture.

## 4. Chapitres

Une suggestion compte les types déjà admis. Elle n'écrit rien. Le standing reste `suggestion`. Zéro groupe publié.

Sur cette fiche, 13 lignes `work` et 2 `publication` franchiraient le seuil de six et proposeraient « Œuvres », avec la justification « faits documentés de type publication, work, creation ». Ce compte mélange la parution de 1820, des clauses sans année et le fragment sur la fille. La suggestion serait explicable, et elle serait trompeuse. Elle n'a pas été publiée. Une vie qui n'a qu'une naissance, dans le test, n'a toujours aucune suggestion.

## 5. Carte et frise

Douai, Paris, Lyon, Bruxelles, Rouen et la France ont reçu un point de localité. « écriture », « L'écriture », Genève et le QID brut sont restés hors carte. Dix-neuf événements n'ont pas de lieu. La fiche explique, quand on ouvre un fait non cartographié, qu'il reste hors de la carte faute de lieu établi ou de géométrie fiable.

La frise est celle de Pixi.js. Les années seules restent des années. Le jour du 30 mai 1845 vient de la phrase sur le procès Crombach. Le jour n'a pas été fabriqué. Plusieurs autres années, elles, ont été fabriquées par le contexte.

## 6. Migration 055

V3.11 ne lit pas `editorial_assertions`. Le refus de `serve` vient du garde-fou général : toute migration embarquée et non appliquée bloque le démarrage, pour qu'un redémarrage ne change pas le schéma.

`055` crée `editorial_assertions` et `editorial_assertion_passages`. Une affirmation `attested` exige un fait logique. Une affirmation `gap` n'en a pas, et ne peut pas citer un passage. Le test `an_assertion_follows_the_passage_across_a_new_version` a réussi sur une base éphémère : après copie du passage sur la version nouvelle, la citation reste `current` ; si la copie disparaît, elle passe `needs_review`. Le fait canonique n'est pas modifié. L'API a démarré sur `talaria_v311b_ephemeral` une fois `055` appliquée.

Préflight pour la base de développement, non exécuté :

1. `max(version)` vaut 54. Ne pas lancer `migrate` tant qu'une autorisation séparée n'a pas été donnée.
2. Avec cette autorisation seulement : sauvegarde, `TALARIA_APPLY_MIGRATIONS=1` sur cette base, puis contrôle que les groupes publiés existants se lisent encore et qu'aucune assertion n'a été créée toute seule.
3. `serve` sans la variable doit alors démarrer, parce qu'il n'y a plus de migration en attente.
4. Ne pas republier de chapitre et ne pas réingérer le corpus pour « remplir » les nouvelles tables. Elles restent vides jusqu'à une rédaction.

## 7. Protections

- Huit recherches actives au plus. Le même QID rejoint le job déjà en cours.
- Istoria demande 8 documents. Le serveur ne suit jamais moins de 8 pages, et l'explorateur reste à 400. Huit pages ont quand même produit 38 faits publics. Le plafond de pages n'est pas un plafond de qualité.
- Pas de limite par adresse, pas d'authentification sur l'ingestion. Un client peut occuper les huit places.
- Trois essais, bail de 90 secondes. Un échec final reste `failed`. Les jobs ne sont pas purgés.
- LLM : 16 tronçons par page si une clé est présente. Cette course n'en avait pas. Il n'y a pas de budget de jetons.
- Wikidata : 2,5 secondes pour la recherche. Les connecteurs de corpus gardent leur propre traitement du 429.

## 8. Tests

- Suggestions de chapitres : 4 tests.
- Place « écriture » refusée.
- Biographie éphémère sans les six publications fictives : une naissance, aucune suggestion.
- Affirmations éditoriales et supersession : passé.
- Vitest recherche, offre d'identité, suggestions : passé.
- `npm run build` : réussi. L'avertissement de taille des chunks est antérieur.
- Playwright : 5 réussites, 1 ignoré. Bureau et mobile.

## 9. Avant une ouverture

Ne pas activer la recherche sur la base de développement tant que :

- `055` n'a pas d'autorisation propre ;
- une page biographique peut encore publier une année absente de la phrase, un pays seul, une notice d'édition ou l'activité d'un proche ;
- l'ingestion publique n'a ni limite par client ni budget.

Le parcours de choix d'identité, lui, peut être relu sur la base éphémère. Elle n'est pas la base de développement et peut être supprimée : `DROP DATABASE talaria_v311b_ephemeral`.
