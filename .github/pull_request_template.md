## Objectif et périmètre
<!-- Décrire le problème, les fichiers/expériences affectés et le résultat attendu. -->

## Changements
<!-- Décrire les modifications de code, de contrat API, de migration et de données. -->

## Invariants Talaria
- [ ] Ne pas inventer d'événements, de sources, de dates ou de coordonnées
- [ ] Respecter la séparation Istoria / Poreia / Agora et les preuves des événements
- [ ] Éviter les refactorisations hors périmètre

## Validation réellement exécutée
<!-- Commandes et résultats. Ne pas annoncer des tests passés sans exécution. -->
- [ ] Tests Rust ciblés (ou justifier l'absence)
- [ ] `cd web && npm test` si frontend concerné
- [ ] `cd web && npm run build` si frontend concerné
- [ ] Test navigateur desktop/mobile si UI concernée
- [ ] Relecture du diff (secrets, caches, modifications non voulues)

## Limites et risques
<!-- Tests non exécutés, dépendances de services, migrations, rollback, impact éventuel sur production. -->

## Flux Git
- Branche de base :
- PR précédente si empilée :
- [ ] CI du niveau actuel vérifiée
- [ ] Pas de fusion/déploiement demandé automatiquement
