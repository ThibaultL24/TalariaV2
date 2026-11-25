# V3.11e — Stabilisation, géolocalisation et préparation de l'activation

Date : 18 novembre 2026. Branche `feat/v311e-activation-prep`, issue de `feat/v311d-ingestion-qualification` (`8d26352`).

La base `talaria_engine_development` n'a pas été écrite. Elle est toujours à la migration 54, avec 3 517 événements canoniques dont 2 218 actifs. La migration 055 n'y a pas été appliquée. Elle a été appliquée sur la base neuve `talaria_v311e_ephemeral`, qui est à la migration 55. Aucun chapitre n'a été publié. Les 2 218 événements actifs n'ont pas été modifiés. Rien n'a été déployé, et `main` n'a pas bougé.

Le moteur n'est pas historiquement infaillible. Ce rapport dit ce qui est démontré sur le même corpus que V3.11d, et ce qui reste une limite.

## 1. Corrections

Quatre corrections de lecture, sans nouvel agent et sans migration nouvelle.

- **Une même publication.** « 1100 exemplaires » n'est plus lu comme une année. La phrase française des Élégies est donc datée de 1819, comme la phrase anglaise. Les deux phrases partagent le titre et l'année : elles deviennent un seul événement. Les Poésies de 1820 restent un autre événement, parce que le titre et l'année diffèrent. Deux œuvres distinctes de la même année ne sont pas fusionnées.
- **La traversée.** Dans « le voyage entrepris… emporte, en mai 1802, la mère », l'année est dans la proposition qui date la mort de la mère. Elle ne date plus le voyage. Le voyage reste public, sans date. Il n'est plus affiché comme un fait de 1802.
- **Les faits retrouvés.** « Paraissent » est reconnu comme une publication : les Poésies de 1820 sont un événement de 1820. « Meurt » est reconnu comme une mort, seulement si la phrase nomme la personne étudiée : la mort du 23 juillet 1859 à Paris précise l'unique mort déjà connue, elle n'en crée pas une seconde. La mort de Junie, dans la phrase sur les enfants, ne devient pas la mort de Marceline. Le Billet reste l'œuvre de Marceline ; la partition de Mees ne suffit plus à écarter la phrase, et 1807 n'est pas copié sur le poème, parce que cette année suit « partition composée par ».
- **La carte.** Un lieu déjà connu du gazetteur n'appelle plus Getty ni le World Historical Gazetteer. Ces deux services restent disponibles seulement si `TALARIA_PLACE_AUTHORITIES=1`. Wikidata garde un délai de 8 secondes et un budget de 8 recherches vivantes par processus. Si la recherche échoue, l'événement reste, sans point. Douai et Bruxelles sont des villes exactes. Stagire et Guernesey sont des lieux nommés à l'échelle d'une cité ou d'une île, pas le centre d'un pays. La France et un lieu seulement approximatif, comme Trafalgar, ne deviennent pas un point. « À l'Odéon puis à Bruxelles » nomme deux lieux : aucun des deux n'est pris comme point de départ unique. Une naissance, une mort, une résidence ou un voyage dont le passage situe l'action peut être cartographié. Le lieu d'une publication reste une mention, souvent une adresse d'imprimeur.

## 2. Comparaison avec V3.11d

Même corpus, base vierge, deux passages. Le second passage ne crée pas d'événement.

| Mesure | V3.11d | V3.11e |
| --- | --- | --- |
| Événements publics | 11 | 13 |
| Erreurs historiques certaines dans ces événements | 0 | 0 |
| Doublon des Élégies | 1 | 0 |
| Traversée datée de 1802 | oui | non |
| Événements sans date | 4 | 4 |
| Événements cartographiés | 0 | 3 |
| Candidats assemblés | 13 | 17 |
| Candidats en revue | 24 | 22 |
| Chapitres publiés | 0 | 0 |
| Suggestions de chapitres | 0 | 0 |

Les quatre événements sans date sont la traversée, le départ vers les théâtres, la carrière anglaise qui cite plusieurs villes, et Le Billet. Leur date absente est le résultat de la lecture, pas un oubli d'enregistrement.

## 3. Inventaire des 13 événements admis

| Fait | Date enregistrée | Lieu cartographié | Lecture |
| --- | --- | --- | --- |
| Naissance | 1786, année exacte | Douai | L'année vient de Wikidata. Le 20 juin est dans la phrase d'ouverture, qui mélange naissance et mort : cette phrase reste en revue. Le jour n'a pas été fabriqué. |
| Mariage | 1817, année exacte | non | Année Wikidata. Pas de lieu démontré sur cette ligne. |
| Élégies et Romances | 1819, année exacte | non | Une seule publication. La phrase française et la phrase anglaise sont réunies. « 1100 exemplaires » n'est pas une date. |
| Poésies | 1820, année exacte | non | Phrase distincte : « En 1820 paraissent les Poésies ». |
| L'Alouette | 1821, année exacte | non | Elle met le poème en musique ; les deux mentions de 1821 sont la même année. |
| Départ du théâtre | après 1823, approximatif | non | « Écriture » reste l'activité, pas un lieu. Le type `office` est large, le fait ne l'est pas. |
| Procès | 30 mai 1845, jour exact | non | La date est dans la phrase. |
| Mort | 23 juillet 1859, jour exact | Paris | La phrase « meurt à Paris, le 23 juillet 1859 » précise la mort Wikidata, qui n'avait que l'année. Une seule mort publique. |
| Départ pour les théâtres | inconnue | non | Odéon puis la Monnaie à Bruxelles. Deux lieux, donc pas un point de départ. |
| Carrière scénique | inconnue | non | Douai, Rouen, Paris et Bruxelles dans la même phrase anglaise. |
| Le Billet | inconnue | non | Le poème est de Marceline. 1807 date la partition de Mees, pas le poème. |
| Résidence | inconnue | Lyon | « Le couple s'installe ensuite à Lyon » situe l'installation. |
| Traversée | inconnue | non | Le voyage est le sien. Mai 1802 date la mort de la mère. |

## 4. Ce qui reste en revue

Vingt-deux candidats restent en revue. Ce sont les notices (Moulin, Bleton, Houssin, Woollett, Revilliod, Seghers), les rues et l'école qui portent son nom, la phrase sur ses parents, la phrase d'ouverture qui réunit naissance et mort, l'inhumation au cimetière de Montmartre, et la ligne structurée de naissance au jour près. Cette dernière n'a pas été publiée à part : la naissance publique reste l'année 1786 à Douai. Aucune de ces lignes n'est un événement public.

La traversée ne reste pas en revue. Elle est publique, sans date. C'est le résultat voulu : l'action est soutenue, la date ne l'est pas.

## 5. Géolocalisation

Le blocage de V3.11d venait d'un appel Getty sans issue utile, après qu'un lieu était déjà résolu. Le gazetteur répond maintenant et s'arrête. Les trois points du corpus sont Douai pour la naissance, Lyon pour l'installation, Paris pour la mort. Bruxelles n'est pas un point de départ. La France n'est pas un point.

Les contrôles hors de ce corpus, sur le gazetteur :

| Lieu | Décision |
| --- | --- |
| Douai | point de ville |
| Lyon, Paris, Arcole, Lodi | point de ville, déjà dans le gazetteur |
| Bruxelles | point de ville si la phrase n'en nomme qu'une |
| Guernesey | point à l'échelle de l'île, pour une résidence qui nomme Guernesey |
| Stagire | point à l'échelle de la cité antique, pas une adresse |
| France, Belgique | refusés : ce sont des pays |
| Trafalgar | refusé : la précision du gazetteur est approximative |

Si Wikidata ne répond pas, le budget s'arrête à 8 recherches et l'événement demeure sans coordonnées. Le délai du client est de 8 secondes. Il n'y a pas de nouvelle tentative illimitée.

## 6. Navigateur

API sur `127.0.0.1:8081`, base `talaria_v311e_ephemeral`. Vite sur `127.0.0.1:5173`. Playwright : 6 tests réussis, 0 ignoré. Le test mobile qui était sauté est exécuté.

Bureau et mobile : la recherche Dumas propose le père et le fils et n'ouvre pas de fiche tant que la recherche n'est pas lancée. Un nom inconnu ne crée pas de fiche. Marceline Desbordes-Valmore ouvre la fiche déjà construite. La timeline contient Lyon et ne contient ni Hyacinthe, ni Gallica, ni un lieu « écriture ». La carte s'affiche. Recharger la fiche ne relance pas le pipeline : aucun job d'ingestion n'a été créé, et le compte est resté à 13.

Le bouton « Lancer la recherche » n'était pas proposé pour Marceline, parce que la fiche existait déjà. Le lancer explicite est vérifié sur Dumas : le bouton est actif, l'adresse ne change pas, et aucun job n'est créé tant qu'on ne le presse pas. Le presser aurait écrit une ingestion vivante ; cela n'a pas été fait.

La carte de cette fiche n'est pas vide : trois lieux sont justifiés. Le texte prévu pour une carte vide est en place : « Aucun lieu n'est assez précis pour être cartographié. Une carte vide ne signifie pas que cette personne n'a jamais voyagé. » Il s'affiche quand la collection géographique est vide. Il ne s'affichait pas ici, parce que la collection ne l'était pas. L'API a été arrêtée après les tests.

## 7. Suggestions de chapitres

Le seuil reste six événements d'un même ensemble. Il n'a pas été abaissé. Sur 13 faits, le groupe le plus nombreux est celui des publications, et il en compte quatre : Élégies, Poésies, L'Alouette, Le Billet. Quatre poèmes ne font pas un chapitre. Abaisser le seuil à quatre ferait apparaître une suggestion réelle, mais aussi une suggestion pour une personnalité dont quatre extraits faibles se ressemblent. Le seuil de six évite cela. Une personnalité peu documentée peut n'avoir aucune suggestion. C'est le cas ici. Aucune suggestion n'est publiée. `editorial_groups` en statut publié : 0.

## 8. Tests

Réussis :

- `talaria-quality` : 158 tests, dont la quantité « 1100 exemplaires », la mort de la mère dans la même phrase, et Le Billet conservé alors que Louise de Caumont reste écartée.
- `talaria-sources`, lieux : 18 tests. Douai est un point, Stagire et Guernesey sont des cités nommées, la France et Trafalgar ne le sont pas.
- Rejeu Desbordes sur base vierge, deux passages, compte stable. Second chargement de `talaria_v311e_ephemeral` : toujours 13.
- `npm run build` réussit. L'avertissement de taille des chunks existait déjà.
- Playwright : 6 réussis, bureau et mobile.

Non exécutés, volontairement : une ingestion vivante de Wikipedia, un flood HTTP des quotas, et toute écriture dans `talaria_engine_development`. Les quotas du lot précédent sont inchangés : 6 recherches par visiteur et par heure, 3 par identité, 4 nouvelles par minute. Les faire tourner contre la base de développement n'était pas permis.

Les avertissements de compilation déjà connus restent : imports inutilisés, `OpenAgendaRow`, attribut de test dupliqué dans `routes/ingest.rs`.

## 9. Procédure d'activation

Cette procédure n'a pas été exécutée. Démarrer le serveur n'autorise pas les recherches qui écrivent.

1. **Base ciblée.** Confirmer que `DATABASE_URL` nomme `talaria_engine_development`, et aucune autre. Au 18 novembre 2026 : migration 54, 2 218 événements actifs, 3 517 lignes au total.
2. **Sauvegarde.** `pg_dump` de cette base, puis restauration d'essai sur une base jetable. Sans restauration réussie, on s'arrête.
3. **Préflight.** `talaria migrate` sans `TALARIA_APPLY_MIGRATIONS`. Il doit refuser tant que 055 est en attente. Aucune autre migration ne doit être présente.
4. **055 seulement.** Avec `TALARIA_APPLY_MIGRATIONS=1` et l'URL confirmée, appliquer 055. Vérifier que la version maximale devient 55 et que le nombre d'événements actifs est toujours 2 218.
5. **Redémarrage.** Relancer l'API. Vérifier qu'elle répond et qu'elle n'a pas relu le corpus.
6. **Contrôles.** Les 2 218 événements actifs sont inchangés. Les identités logiques existantes ne sont pas fusionnées. `editorial_groups` publié ne gagne aucune ligne.
7. **Recherche limitée.** L'ingestion automatique de l'explorateur, à 400 documents, reste fermée. Seul le bouton Istoria peut lancer une recherche, sur une personnalité absente de la base. Quotas inchangés. `TALARIA_PLACE_AUTHORITIES` reste vide : pas d'appel Getty sur le chemin courant. `TALARIA_GEOCODE_BUDGET` reste 8.
8. **Publication.** Un fait entre dans la biographie seulement s'il est admis. Une date inconnue reste inconnue. Un lieu non justifié ne devient pas un point. Les suggestions de chapitres ne sont pas publiées.
9. **Désactivation.** Retirer le bouton, ou refuser `POST` d'ingestion, puis redémarrer. Les événements déjà admis restent. On ne relance pas le pipeline pour « nettoyer ».
10. **Faux événement.** S'il en paraît un : noter l'identité, le passage et l'identifiant. Désactiver la recherche. Ne pas rejouer l'ingestion sur les 2 218. Corriger la règle, puis rejouer la personnalité sur une base jetable. La ligne fautive de la base de développement est retirée par une action explicite, pas par une republication.
11. **Après coup.** Relire les événements nouveaux un par un. Vérifier qu'aucun job n'a touché une identité déjà présente. Vérifier que 055 n'a pas été rejouée.

Activer la recherche ne republie pas les anciens essais. Les 38 lignes de V3.11c ne sont pas dans cette base. Il ne faut pas réingérer les personnalités déjà présentes pour « profiter » des règles nouvelles.

## 10. Verdict

**Activation contrôlée recommandée.**

Les conditions demandées :

- Des erreurs historiques certaines dans les faits publics : non, sur les 13 événements relus.
- Le doublon des Élégies : absent de la nouvelle ingestion.
- La traversée : publique, sans date. Elle n'est pas présentée comme un fait de 1802. Elle n'est pas laissée en revue, parce que le voyage lui-même est soutenu.
- Les dates et les intervalles : l'année, le jour, « après 1823 » et l'absence de date sont conservés tels que le passage les porte.
- Les géométries : Douai, Lyon et Paris sont dans le passage de l'action. Aucun pays n'est un point. Bruxelles n'est pas un lieu de départ inventé.
- Les quotas : inchangés, et aucune recherche d'écriture n'a été lancée depuis le navigateur.
- Le parcours mobile : exécuté, 6 tests sur 6.
- La migration 055 : elle s'applique sur une base neuve. Elle n'est pas sur la base de développement.
- La base de développement peut être activée sans modification non autorisée : oui, en suivant la procédure ci-dessus, qui n'a pas été lancée.

Ce qui n'est pas acquis : le jour de naissance reste en revue parce qu'il partage sa phrase avec la mort ; le type « office » du départ du théâtre est plus large que le fait ; une carte peut rester vide si le gazetteur et Wikidata ne connaissent pas le lieu, et cette carte vide ne veut pas dire que la personne n'a pas voyagé. L'activation porte sur la recherche explicite, bornée, d'une personnalité absente. Elle ne porte pas sur une republication du corpus déjà en base.
