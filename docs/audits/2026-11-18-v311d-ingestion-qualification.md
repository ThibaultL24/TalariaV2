# V3.11d — Qualification du moteur avant activation

Date : 18 novembre 2026. Branche `feat/v311d-ingestion-qualification`, issue de `feat/v311c-public-promotion` (`fa16586`).

La base `talaria_engine_development` n'a pas été écrite. Elle est toujours à la migration 54, avec 3517 événements canoniques. La migration 055 n'y a pas été appliquée. Elle a été appliquée seulement sur la base neuve `talaria_v311d_ephemeral`. Aucun chapitre n'a été publié. Aucune archive n'a été réécrite. Rien n'a été déployé, et `main` n'a pas bougé.

Les chiffres 29 / 8 / 1 du lot précédent mesurent l'ancien corpus de 38 événements. Ils ne mesurent pas ce moteur.

## 1. Référentiel des 38 événements

Le rapport V3.11c reste la source du classement initial. Il n'est pas réécrit. Sur ces 38 lignes : 29 n'étaient pas justifiées par leur passage, 8 étaient potentiellement conservables, 1 restait incertaine (la traversée dont « mai 1802 » date la mort de la mère). Les 8 ne sont pas tenues pour approuvées : elles ont été relues avec les autres.

Un fait est conservable seulement si l'action et l'attribution tiennent. La date peut rester inconnue. Le lieu peut rester inconnu. L'absence de géométrie ne retire pas le fait.

## 2. Trois décisions séparées

Le code les rend indépendantes.

| Décision | Règle | Test |
| --- | --- | --- |
| Admissibilité | Le passage soutient l'action de la personnalité. Une notice, un proche, une rue « qui porte son nom » ou une citation sans verbe restent en revue. | `a_street_named_after_her_is_not_her_act`, `a_bibliography_line_is_not_her_act` |
| Temps | Une date n'est enregistrée que si le passage la porte. Une année devinée, y compris par l'extracteur, n'est pas stockée. « Après 1823 » reste approximatif. L'absence de date ne renvoie plus `NeedsReview` à elle seule. | `unknown_time_does_not_by_itself_withhold_the_fact`, `a_missing_date_or_point_does_not_change_the_historical_decision` |
| Carte | Un pays, deux localités dans la même phrase, ou le titre d'un poème ne deviennent pas un point. Sans géométrie, `map_eligible` reste faux et l'événement reste dans la biographie. | `a_country_name_does_not_become_a_map_point`, `a_poem_title_is_not_a_place` |

Une lettre sans date ni lieu (« Marceline écrit à Prosper Lanchantin… ») est écrite dans `canonical_events` avec `time_json.kind = unknown` et `map_eligible = false`. C'est le cas du test `doubtful_lines_do_not_enter_the_public_biography`.

## 3. Rejeu du corpus initial

Base neuve, puis le même rejeu dans `talaria_v311d_ephemeral`. Documents : les extraits français et anglais déjà utilisés, et les deux fiches Wikidata du test. Les connecteurs de catalogue ne sont pas dans ce corpus : les 38 anciennes lignes publiques ne les citaient pas.

Chaîne exercée : extraction des règles, ancrage, porte, persistance. Le modèle de langue est écarté pour que le résultat ne dépende pas d'un appel. Le géocodage en direct (Getty, Wikidata) est écarté par `TALARIA_OFFLINE_PLACES=1` : un appel sans délai vers le thésaurus Getty bloquait le rejeu. Le client Wikidata a maintenant un délai de 8 secondes. Les coordonnées de ce lot viennent donc seulement du gazetteer local.

Un second passage sur les mêmes textes ne crée pas de nouvel événement public.

| Mesure | Valeur |
| --- | --- |
| Candidats assemblés | 13 |
| Candidats en revue | 24 |
| Événements publics | 11 |
| Erreurs certaines parmi ces 11 | 0 |
| Incertains | 1 |
| Doublon | 1 |
| Chapitres publiés | 0 |
| Suggestions de chapitres | 0 |
| Points cartographiques | 0 |

Les extraits Wikipédia français et anglais relus en direct le 18 novembre ont le même texte que les fichiers du rejeu (23 417 et 2 461 caractères, mêmes empreintes). Il n'y a pas d'écart documentaire à enregistrer pour ces deux pages. Les autres pages liées n'ont pas été recrawlées.

## 4. Les 11 événements publics

| Fait | Passage | Date | Lieu | Décision |
| --- | --- | --- | --- | --- |
| Naissance | énoncé Wikidata `born_in 1786` | exacte, année 1786 | Douai, non cartographié | Conservé. L'année est dans l'énoncé. Douai n'a pas de géométrie dans le gazetteer local. |
| Traversée | « Le voyage entrepris… en mai 1802, la mère… » | exacte, année 1802 | aucun | Incertain. 1802 est dans la phrase, mais il date la mort de la mère pendant le voyage. |
| Mariage | énoncé Wikidata `married 1817` | exacte, année 1817 | le QID n'est plus un lieu | Conservé. Année justifiée, lieu non établi. |
| Publication | « published Élégies et Romances… in 1819 » | exacte, année 1819 | aucun | Conservé. |
| Départ du théâtre | « Après 1823… quitte définitivement le théâtre… écriture » | approximative, « après 1823 » | aucun | Conservé. La borne ouverte n'est plus une date exacte, et « écriture » n'est plus un lieu. Le type `office` est trop large. |
| Procès | « intervient dans le procès le 30 mai 1845 » | exacte, jour | aucun | Conservé. Elle intervient ; Crombach est l'accusé. |
| Mort | énoncé Wikidata `died_in 1859` | exacte, année 1859 | aucun | Conservé au niveau de l'année. La phrase « meurt à Paris, le 23 juillet 1859 » reste en revue : le jour et Paris ne sont pas publiés. |
| Départ vers la scène | « quitte définitivement son amant… Odéon puis… Bruxelles » | inconnue | Bruxelles, non cartographié | Conservé. L'année 1808 n'est plus inventée. Bruxelles est un des deux théâtres, donc pas un point. |
| Carrière scénique | « appeared… in Douai, Rouen… Paris… Brussels » | inconnue | aucun | Conservé comme un seul fait. L'ancien lot en faisait quatre événements datés de 1817 et cartographiés. |
| Publication | « En 1819… Élégies et Romances… En 1820 paraissent les Poésies » | inconnue | aucun | Doublon du recueil de 1819. La date reste inconnue parce que la phrase porte deux années. Poésies, 1820, n'est pas devenu un événement séparé. |
| Résidence | « Le couple s'installe ensuite à Lyon » | inconnue | Lyon, non cartographié | Conservé. Le fait tient sans date et sans coordonnées. |

Précision sur cet échantillon : 9 faits tenus, 1 doublon, 1 incertain, 0 erreur certaine. Les cas incertains ne sont pas comptés comme des succès.

Rappel sur les 8 lignes autrefois jugées conservables : naissance, mariage (fiche), Élégies, procès et mort (année) sont là. Le mariage en prose n'a pas été réextrait à part. La mort du 23 juillet et Poésies de 1820 ne sont pas des événements publics distincts. Le Billet, 1807, reste en revue : la phrase dit aussi que la partition est composée par Joseph-Henri Mees, et la règle d'attribution retient le compositeur. L'Alouette est assemblée sans devenir une ligne publique séparée, et « alouette » n'est pas un lieu.

Ce n'est pas un objectif de huit événements. Des faits supplémentaires justifiés sont entrés : la résidence à Lyon sans date, le départ vers Bruxelles sans date, la sortie du théâtre « après 1823 », et la carrière scénique en un seul fait non daté.

## 5. Comparaison

| | V3.11b | V3.11c sur les mêmes phrases | V3.11d, rejeu complet |
| --- | --- | --- | --- |
| Public | 38 | les cinq erreurs imposées ne passaient plus, sans rejeu de la page | 11 |
| Erreurs certaines | 29 | bloquées phrase par phrase | 0 dans les 11 |
| Incertains | 1 | la traversée de 1802 | 1, la même traversée |
| Faits sans date conservés | non, une année était souvent inventée | la porte refusait encore tout temps inconnu | oui : Lyon, Bruxelles, carrière |
| Carte | 12 points, dont France et des villes héritées | un pays n'est plus un point | 0 point dans ce rejeu hors ligne |

## 6. Faits corrects conservés

Neuf, listés ci-dessus, plus la lettre sans date du test d'intégration, qui n'est pas dans la page Wikipédia. Les faits incomplets ne sont pas écartés pour manque de date ou de coordonnées.

## 7. Faux événements bloqués

Ne réapparaissent pas : Hyacinthe, Louise de Caumont, Henri Woollett, la notice Gallica de 1873, Houssin, les mentions d'imprimerie, Jeanine Moulin, le médaillon de David d'Angers, les rues « qui portent son nom », le retour en France daté de 1786, le pays France comme point, le lieu « écriture », les quatre villes de 1817 séparées et cartographiées, le fragment « et à Paris ».

## 8. Nouveaux faux positifs

Aucun événement public de ce rejeu n'est une erreur certaine. Le doublon français des Élégies n'invente pas de date. Le type `office` pour quitter le théâtre est une étiquette trop large, pas un fait faux.

## 9. Pertes de rappel

- « meurt à Paris, le 23 juillet 1859 » reste en revue alors que la mort annuelle est publique.
- « En 1820 paraissent les Poésies » n'est pas séparé du paragraphe de 1819.
- Le Billet, 1807, reste en revue à cause du compositeur nommé dans la même phrase.
- « Marceline part rejoindre son père » n'est pas republié. L'ancienne date 1802 n'est plus collée sur cette phrase.

## 10. Dates

Aucune année publique de ce rejeu ne vient de la naissance ou d'un paragraphe voisin quand la phrase ne la contient pas. « Après 1823 » est `approx`, surface « après 1823 ». Lyon et Bruxelles n'ont pas de date. La traversée de 1802 reste le seul cas où l'année de la phrase ne porte pas clairement l'action retenue.

## 11. Lieux

Aucun des 11 événements n'est `map_eligible`. Douai, Bruxelles et Lyon sont nommés quand la phrase les nomme, sans point. France, « écriture » et « alouette » ne sont pas des lieux. Deux théâtres dans une phrase ne donnent pas un point. Ce zéro vient du gazetteer local : le rejeu n'a pas appelé Wikidata pour les coordonnées. Un lieu sans géométrie est resté dans la biographie.

## 12. Chapitres

Aucune suggestion. Le seuil est de six événements d'une même famille, et aucune famille n'y arrive. Un chapitre « Œuvres » n'a pas été proposé à partir des notices de catalogue : ces notices sont en revue. Rien n'a été écrit dans `editorial_groups`.

## 13. Navigateur

API sur `127.0.0.1:8081`, base `talaria_v311d_ephemeral`, puis Playwright (Chromium). Bureau et Pixel 5.

Cinq tests passent, le rejeu mobile de la recherche est sauté comme auparavant. Le parcours bureau ouvre la fiche Desbordes-Valmore, la frise Pixi, la carte MapLibre, recharge la fiche, et affiche que la recherche n'est pas une vérification historique. Il n'y a pas de libellé « Biographie vérifiée ». Le JSON de la frise contient Lyon, ne contient ni Hyacinthe ni Gallica, et aucun `place_label` n'est « écriture ». Le mot « écriture » reste dans la phrase sur le théâtre, où il désigne l'activité, pas un lieu. La sélection Dumas distingue le père et le fils sans lancer de recherche. Un nom sans fiche Wikidata ne crée pas de fiche.

Le bouton de recherche n'a pas créé de job pendant ce passage : la fiche venait déjà du rejeu, et le compteur est resté à 11.

## 14. Quotas et abus

Limites déjà en base, donc visibles d'un processus à l'autre : 3 recherches par personnalité et par heure, 4 nouvelles recherches par minute, 8 recherches simultanées. Une clé visiteur est maintenant envoyée par le navigateur (`localStorage`, `talaria.visitor`) et comptée dans `request_json`, sans migration. Plafond : 6 recherches par heure pour une clé d'au moins 8 caractères. Le test `a_visitor_quota_is_visible_to_a_second_connection` écrit trois jobs terminés et les relit depuis une seconde connexion du même pool. Changer de clé ne lève pas le plafond par personnalité. Les réponses `429` portent `ingest_quota` avec le périmètre `identity`, `burst` ou `visitor`. Ce lot n'a pas rejoué une rafale HTTP complète : la persistance du compteur visiteur est prouvée en base, pas par un deuxième processus lancé à la main.

Le modèle reste plafonné à quatre tronçons par page quand une clé est présente. Ce rejeu n'a pas appelé de modèle.

## Verdict

**Activation possible avec restrictions.** Pas une activation libre.

Le rejeu complet, sur une base vierge et les mêmes pages, ne republie pas les erreurs certaines de l'ancien lot. Il garde des faits sans date et sans coordonnées quand l'action est étayée. La carte de ce lot est vide, ce qui est cohérent avec l'absence de géométrie, pas avec un rejet des faits.

Restrictions avant toute ouverture sur `talaria_engine_development` :

- Ne pas présenter une recherche terminée comme une biographie vérifiée. L'interface le dit déjà.
- La traversée de 1802 reste ambiguë.
- Le jour de la mort, Poésies de 1820 et Le Billet manquent encore comme événements distincts.
- Les coordonnées de ce certificat viennent du gazetteer local. Un géocodage en direct devra être borné par le délai déjà ajouté, et revu à part.
- Les chapitres restent des suggestions. Ici, il n'y en a aucune.
- La migration 055 et la recherche universelle sur la base de développement ne font pas partie de ce lot.

Ce verdict vient des 11 lignes relues, pas du seul fait que les tests passent.
