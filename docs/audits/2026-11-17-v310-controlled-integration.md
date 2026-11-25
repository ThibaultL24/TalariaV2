# V3.10 — revue historique et intégration contrôlée

Date : 17 novembre 2026. Branche `feat/v310-controlled-integration`, issue de `feat/v39-targeted-recovery` (`58f4538`).

Aucune ligne de `talaria_engine_development` n'a été modifiée. La migration `055` n'a pas été appliquée. Aucun chapitre n'a été publié. Une décision technique `Accept` n'est pas une approbation éditoriale.

## Décision

| Événement | Preuve directe | Date | Lieu | Attribution | Doublon | Décision |
|---|---|---|---|---|---|---|
| Bataille d'Arcole | Oui. La seconde phrase dit que la manœuvre est celle de l'armée de Bonaparte. | 15–17 novembre 1796, intervalle au jour | Arcole, lieu de l'action | Commandement | Non. Lonato est un autre combat, les 3 et 4 août, lieu enregistré « Italy ». | Accept technique. Approbation humaine en attente. |
| Bataille de Castiglione | Oui. « under General Napoleon Bonaparte attack … on 5 August 1796 ». | 5 août 1796, jour exact | Castiglione, lieu de l'action | Commandement | Non. Absent du corpus `person`. | Accept technique. Approbation humaine en attente. |
| Seconde bataille de Dego | La date et le lieu sont dans la phrase. Le nom de Bonaparte n'y est pas. | 14–15 avril 1796, intervalle au jour | Dego | Non démontrée | Non, mais cela ne suffit pas. | NeedsReview |
| Siège de Mantoue | Oui. « under the overall command of Napoleon Bonaparte ». | 4 juin 1796 – 2 février 1797, les deux bornes au jour | Mantoue, lieu de l'action | Commandement | Non. Absent du corpus `person`. | Accept technique. Approbation humaine en attente. |
| Armistice de Cherasco | Oui. La trêve est signée entre Victor-Amédée III et Bonaparte. | 28 avril 1796, jour exact | Cherasco, lieu de la signature, pas un champ de bataille | Signataire | Non. Absent du corpus `person`. | Accept technique. Approbation humaine en attente. |
| Traité de Campo Formio | La phrase d'ouverture nomme la République et l'Autriche. Elle ne nomme pas Bonaparte. | 17 octobre 1797, jour exact, si la phrase est retenue | Campo Formio, lieu de la signature | Non démontrée | Non. Absent du corpus `person`. | NeedsReview |

Contrôles tenus hors de la liste : Ceva (`NeedsReview`, l'action nomme Augereau) et Tolentino (`NeedsReview`, l'action nomme Murat ; « Napoleonic » n'est pas le nom Napoléon).

## 1. Dossiers

Les passages sont ceux de la reprise V3.9 et des tests de cette branche. La proximité du nom dans la page ne compte pas comme une participation. Personne : Napoléon Bonaparte, Q517. Les événements n'ont pas d'identifiant canonique Talaria : ils ne sont pas dans `canonical_events`.

### Bataille d'Arcole

- Document : page anglaise « Battle of Arcole », aussi appelée Arcola.
- Type : `battle`. Action : combat. Rôle retenu : commandement, parce que la phrase dit « a bold maneuver by Napoleon Bonaparte's French Army of Italy ».
- Date : intervalle 15–17 novembre 1796, précision jour, les deux bornes dans la même année.
- Lieu : Arcole. Le rôle géographique est le lieu de l'action. Vérone n'est que le repère « southeast of Verona ».
- Passage : « The Battle of Arcole (15–17 November 1796) was fought between French and Austrian forces southeast of Verona. The battle saw a bold maneuver by Napoleon Bonaparte's French Army of Italy. »
- Contexte : campagne d'Italie, après Castiglione, pendant que le siège de Mantoue continue.
- Contradiction : le nom Arcola désigne le même combat, pas un second événement.
- Voisins : Lonato (3–4 août 1796, lieu stocké « Italy ») et Bassano (1796) sont déjà canoniques. La date, le lieu et la phrase les séparent d'Arcole.
- Distinct : oui, si la phrase de commandement est la preuve retenue.

### Bataille de Castiglione

- Document : « Battle of Castiglione ».
- Type : `battle`. Action : attaque. Rôle : commandement.
- Date : 5 août 1796, jour exact. Ce n'est pas un intervalle.
- Lieu : Castiglione.
- Passage : « The Battle of Castiglione saw the French Army of Italy under General Napoleon Bonaparte attack the Austrian army on 5 August 1796. »
- Contexte : le siège de Mantoue a déjà commencé (4 juin). Castiglione n'est pas une date du siège.
- Voisin piégeux : Lonato a lieu les 3 et 4 août 1796. Le jour suivant et le lieu différent interdisent de les confondre.
- Distinct : oui.

### Seconde bataille de Dego

- Document : « Second Battle of Dego ». La page courte « Battle of Dego » ne portait pas le passage daté en V3.9.
- Type proposé : `battle`. Action : combat.
- Date : 14–15 avril 1796, intervalle au jour.
- Lieu : Dego.
- Passage daté : « The Second Battle of Dego was fought on 14 and 15 April 1796 near Dego. »
- Attribution : cette phrase ne dit pas qui commande. Elle reste en revue. Une participation de Bonaparte n'est pas ajoutée.
- Distinct du point de vue du lieu et de la date, insuffisant du point de vue de la personne.

### Siège de Mantoue

- Document : « Siege of Mantua (1796–1797) ».
- Type : `siege`. Action : siège. Rôle : commandement d'ensemble.
- Date : du 4 juin 1796 au 2 février 1797. Précision jour sur les deux bornes. La surface et `time_json` gardent l'intervalle. `start_time` reste la projection d'ordre au 4 juin 1796, pas la date sémantique.
- Lieu : Mantoue.
- Passage : « The siege of Mantua lasted from 4 June 1796 to 2 February 1797 under the overall command of Napoleon Bonaparte. »
- Contexte : Castiglione (5 août 1796) et Arcole (15–17 novembre 1796) tombent pendant cet intervalle. Ce sont d'autres faits, pas des doublons du siège.
- Avant ce correctif, la phrase devenait l'année 1796 seule, parce que le texte entre 1796 et 1797 (« to 2 February ») n'était pas reconnu comme un intervalle.

### Armistice de Cherasco

- Document : « Armistice of Cherasco ».
- Type : `diplomatic`. Action : trêve signée. Rôle : signataire, pas commandant d'une bataille.
- Date : 28 avril 1796, jour exact.
- Lieu : Cherasco, lieu de la signature.
- Passage : « The Armistice of Cherasco was a truce signed at Cherasco on 28 April 1796 between Victor Amadeus III of Sardinia and Napoleon Bonaparte. »
- L'autre partie est Victor-Amédée III. Le rôle de Bonaparte est celui d'une partie à la trêve, tel que la phrase le dit.
- Distinct : oui. Ce n'est pas la bataille de Dego ni un séjour.

### Traité de Campo Formio

- Document : « Treaty of Campo Formio ».
- Type proposé : `treaty`. Action : paix signée.
- Date lisible dans l'ouverture : 17 octobre 1797, jour exact.
- Lieu : Campo Formio, lieu de la signature.
- Passage utilisé : « The Treaty of Campo Formio was a peace treaty signed on 17 October 1797 between the French Republic and Austria. »
- Attribution : les parties nommées sont deux États. Bonaparte n'est pas dans cette phrase. Le rôle personnel reste en revue. Une signature n'est pas inventée à partir du titre.

### Lecture du corpus réel

Une lecture seule sur `talaria_engine_development` (`SET default_transaction_read_only = on`) ne trouve aucun titre `person` actif pour Arcole, Castiglione, Mantoue, Dego, Cherasco, Campo Formio, Ceva ou Tolentino. Deux titres longs contiennent la suite de lettres « ceva » dans des mots français (« décevante », « recevant ») : ce ne sont pas la bataille de Ceva.

## 2. Intervalles

`TypedTime::Range` conserve désormais, en plus des années, le mois et le jour de chaque borne quand la phrase les donne. `time_json` écrit `kind: range`, `precision: day`, `start: 1796-06-04`, `end: 1797-02-02`. Une plage d'années sans mois reste `precision: year` et la clé `range:{début}:{fin}` déjà utilisée.

Lectures couvertes par les tests :

- Mantoue, 4 juin 1796 – 2 février 1797.
- Arcole, 15–17 novembre 1796.
- Dego, 14 et 15 avril 1796.
- Castiglione et Cherasco restent des jours exacts.
- « c. 1797 » et « from 1796 » restent approximatifs, sans borne de fin inventée.
- « date inconnue » et l'absence de surface restent inconnues.
- « In December 1805 … introduced in 1793 » reste ambigu. Une parenthèse « (20–21 March) » sans année reste sans date.

Istoria : un événement `kind: range` affiche les deux bornes, ou la surface entière, au lieu de la seule année de début. La frise Pixi place le point au début et trace un segment jusqu'à la fin. Un événement long reste visible si la fenêtre coupe son intervalle. La fenêtre d'années inclut l'année de fin. La carte ne reçoit un point que si une géométrie existe : le test refuse `map_eligible` quand `geom` est nul. Arcole, Castiglione, Mantoue et Cherasco ne sont pas dans le gazetteer embarqué ; la simulation ne leur a pas inventé de coordonnées.

## 3. Attribution

Le modèle déjà présent est réutilisé. `ParticipantRole` gagne les valeurs qui manquaient pour dire le rôle lu dans la phrase : `commander`, `signatory`, `addressee`, `contextual`. Il n'y a pas de nouvelle table.

Une page de bataille suivie n'est plus acceptée toute seule. `demonstrated_role` ne retient un rôle que si le mot du sujet est entier et que la phrase le place dans l'action :

- commandement : « under … Napoleon », « command of Napoleon », « by Napoleon », « Napoleon's army » ;
- signature : « signed … and Napoleon », « Napoleon signed » ;
- « Napoleonic » ne contient pas le mot Napoléon ;
- une phrase qui nomme un autre général, sans le sujet, ne crée pas de participation.

Arcole, avec la phrase de la manœuvre, est un commandement. Ceva, qui nomme Augereau, reste en revue. Tolentino, qui nomme Murat, reste en revue. Austerlitz cité sans le nom du sujet n'est plus un accept automatique.

## 4. Candidats

Le chemin existant est `event_candidates`, puis `canonical_events` seulement si la porte est `Accept` et si l'attribution est auto-acceptée. `MentionedNotActing` donne `NeedsReview` et n'écrit pas de ligne canonique.

| Contrôle | Résultat technique |
|---|---|
| Source narrative, pas une notice de catalogue | Oui pour les six phrases retenues |
| Action | Combat, siège, trêve ou traité, selon la phrase |
| Date | Intervalle ou jour, jamais une année substituée |
| Lieu | Le lieu de l'action ou de la signature, pas un royaume |
| Personne | Voir le tableau |
| Doublon | Aucun des six n'est Lonato ni un titre déjà actif |
| Preuve directe | Quatre phrases montrent le rôle. Deux ne le montrent pas. |
| Identité | `occurrence_key` inclut le type, le rôle, les deux bornes, le lieu et l'objet. L'identifiant d'extracteur n'en fait pas partie. |

## 5. Simulation éphémère

`reviewed_italian_events_persist_once_on_a_fresh_database` applique les migrations sur une base jetable, puis appelle `persist_fact_item`.

| Mesure | Avant | Après la première passe | Après la seconde passe |
|---|---|---|---|
| Événements canoniques actifs | 0 | 4 | 4 |
| Preuves | 0 | 4 | 4 |
| `logical_event_id` distincts | 0 | 4 | 4 |
| Dego, Campo Formio, Ceva, Tolentino canoniques | 0 | 0 | 0 |

Les quatre lignes sont Arcole, Castiglione, Mantoue et Cherasco. Mantoue est stockée `1796-06-04` → `1797-02-02`, précision jour. Castiglione est `1796-08-05`, kind exact. La seconde passe ne crée ni événement, ni preuve, ni identité logique supplémentaire.

La base de développement n'a pas reçu ces lignes. La lecture API vérifiée est la forme `time_json` que la frise consomme déjà (`kind`, `precision`, `start`, `end`). Le serveur HTTP n'a pas été relancé : ces faits ne sont pas dans la base que l'explorateur lit.

## 6. Chapitres

Aucun chapitre n'est publié. Le corpus réel est inchangé : zéro de ces six faits y est exploitable aujourd'hui. La simulation en rend quatre utilisables pour une relecture, toujours sans approbation humaine.

Campagne d'Italie, si les quatre accept techniques sont un jour approuvés : Dego resterait dehors. L'ordre des faits datés serait Cherasco (28 avril, diplomatie), ouverture du siège de Mantoue (4 juin), Castiglione (5 août), Arcole (15–17 novembre), fin du siège (2 février 1797). Le siège couvre Castiglione et Arcole ; il doit rester une période, pas un point en juin.

Diplomatie : Cherasco peut entrer comme trêve signée par Bonaparte, au rôle que la phrase donne. Campo Formio ne peut pas être attribué à Bonaparte tant que la phrase retenue ne le nomme pas.

## 7. Anecdotes

Aucune n'est approuvée. Une seule source ne les rejette pas. Le niveau vient de la phrase et de l'absence de revue humaine. Les passages sont ceux relevés en V3.9.

### Lettre de Victor Hugo sur John Brown

- Affirmation : en 1859, Hugo écrit une lettre au sujet de John Brown.
- Passage : « In 1859, he wrote a letter… » (page anglaise Victor Hugo, Q535).
- Date : 1859. Lieu : non établi par cette phrase.
- Intérêt : geste public pendant l'exil.
- Incertitude : le texte cité en V3.9 est le début de la phrase, pas la lettre elle-même.
- Controverse : le sujet politique de la lettre demande une lecture du texte, pas un rejet automatique.
- État : documenté, en revue. Pas approuvé.

### Départ vers Jersey

- Affirmation : après le coup d'État de 1851, Hugo passe par Bruxelles puis s'installe à Jersey.
- Passage : « He moved to Brussels, then Jersey… »
- Date : après 1851, sans jour dans la phrase retenue. Lieu : Jersey.
- Intérêt : c'est un déplacement biographique, pas une anecdote narrative.
- Incertitude : une occurrence « Guernesey » dans la même page est une dédicace de roman, pas ce départ.
- État : documenté comme fait de résidence, en revue. Pas approuvé. Ne pas le classer comme anecdote.

### Marie Curie gouvernante

- Affirmation : elle est gouvernante pendant deux ans à Szczuki, chez les Żorawski.
- Passage : « for two years as a governess in Szczuki … the Żorawskis. » (Q7186).
- Date : durée sans année dans la phrase. Lieu : Szczuki.
- Intérêt : épisode biographique ordinaire, pas un récit à chute.
- Incertitude : l'année manque. Une seule source ne suffit pas à fixer la date.
- État : candidate, en revue. Pas approuvée.

### Marceline Desbordes-Valmore

- Fait d'état civil, distinct de l'anecdote : « née le 20 juin 1786 à Douai et morte le 23 juillet 1859 à Paris. » Documenté, pas une anecdote, pas approuvé.
- Anecdote candidate : « la traversée … devient un calvaire ». Date et lieu du voyage ne sont pas fixés par le fragment relu. Le caractère pathétique est dans le mot choisi par la source.
- État : candidate narrative, en revue. Pas approuvée. Ne pas la fusionner avec la naissance.

## 8. Plan d'intégration

À ne pas exécuter sans une autorisation explicite qui nomme la liste. La migration `055` reste hors de cette opération.

1. Sauvegarde : `pg_dump` de `talaria_engine_development` avant toute écriture. Pas de `TRUNCATE`. Pas de `reset`.
2. Schéma : confirmer que les migrations appliquées s'arrêtent avant `055_editorial_assertions.sql`. Ne pas lancer `talaria migrate` si cela applique 055. Recompiler `talaria-store` seulement si une migration autorisée est ajoutée. Ce plan n'en ajoute pas.
3. Préflight en lecture seule : répéter le contrôle de titres ci-dessus. Calculer les `occurrence_key` des quatre faits avec le type, le rôle, les deux bornes, le lieu et l'objet. Vérifier qu'aucune clé active de Q517 ne coïncide, en particulier celle de Lonato.
4. Liste autorisée, et rien d'autre :
   - Arcole, bataille, 15–17 novembre 1796, commandement, phrase de la manœuvre.
   - Castiglione, bataille, 5 août 1796, commandement, phrase « under General Napoleon Bonaparte ».
   - Mantoue, siège, 4 juin 1796 – 2 février 1797, commandement, phrase « overall command ».
   - Cherasco, diplomatique, 28 avril 1796, signataire, phrase de la trêve.
   Exclus : Dego, Campo Formio, Ceva, Tolentino.
5. Transaction unique via le chemin `person` déjà testé (`persist_fact_item`). En cas d'erreur, annulation de la transaction. Ne pas incrémenter `source_count`. Une preuve déjà présente est un `ON CONFLICT DO NOTHING`.
6. Invariants : le nombre d'événements actifs de Q517 augmente exactement du nombre de clés nouvelles. Les preuves des quatre documents existent. Les quatre `logical_event_id` sont distincts et différents de celui de Lonato (`472a66c3-2775-45cd-9660-fcaf7afb70a8`). Une seconde exécution de la même liste ne change aucun de ces comptes.
7. Retour arrière sans effacer les preuves : `supersede` de la ligne publiée à tort. L'ancienne ligne devient inactive. Les preuves restent attachées. Pas de suppression de `event_evidence`.
8. Après écriture seulement : relire la frise et la carte. Mantoue doit apparaître comme une période. La carte ne montre un point que si la géométrie est réelle.

## 9. Tests

- `cargo test -p talaria-quality --lib` : 134 tests, verts.
- `cargo test -p talaria-quality --tests` : 13 tests d'intégration, verts.
- `cargo test -p talaria-api --bin talaria -- reviewed_italian_events` : vert, base jetable, 50 s.
- Vitest : `timeline-utils`, `event-display`, `chapter-reading`, 20 tests verts.
- `cd web && npm run build` : vert. Avertissement déjà connu sur la taille des chunks.

## Correctifs

- Un intervalle « from 4 June 1796 to 2 February 1797 » n'est plus réduit à 1796.
- Un combat de plusieurs jours dans le même mois garde ses deux jours.
- Une page militaire suivie sans rôle du sujet reste en revue.
- « Napoleonic » n'attribue pas l'action à Napoléon.
- La frise et Istoria peuvent montrer la fin d'une période, pas seulement son début.
