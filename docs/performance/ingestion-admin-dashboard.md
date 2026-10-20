# Talaria — Administration des ingestions

L'interface est accessible à `/admin/ingestions` après connexion via une session Talaria valide (portefeuille/SIWE). **Un compte connecté n'est pas automatiquement administrateur.**

## Activation sécurisée

Configurer côté serveur uniquement `TALARIA_ADMIN_USER_IDS` avec la liste explicite (séparée par des virgules) des UUID **internes** des utilisateurs autorisés. Ces UUID sont ceux retournés dans `GET /api/v1/auth/me` après connexion. Ne pas y mettre les adresses publiques des portefeuilles, QID Wikidata, tokens de session ou secrets.

Exemple *fictif* :
```env
TALARIA_ADMIN_USER_IDS=aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa
```

Sans variable ou avec une liste vide/invalide, tout accès est refusé (**fail closed**). Les comptes sans session obtiennent HTTP 401, les comptes connectés non autorisés HTTP 403. La même vérification serveur protège :
- `GET /api/v1/admin/ingestions/summary?days=30`
- `GET /api/v1/admin/ingestions/jobs?days=30&limit=25&offset=0&lane=agora&status=done`
- `GET /api/v1/ingest/{job_id}/metrics` (ancien endpoint auparavant public, désormais privé)

L'interface n'est pas référencée dans la navigation publique. Ce n'est pas une mesure de sécurité ; toutes les barrières sont en Rust, vérifiées depuis le cookie de session HttpOnly. Ne pas exposer `TALARIA_ADMIN_USER_IDS` dans Vite.

## Métriques

Le backend calcule les percentiles P50/P95 des durées d'ingestion et des tentatives fournisseur sous PostgreSQL, plus les volumes et incidents (timeouts, HTTP 429, échecs) par source. Les taux de liaison reposent sur des paires distinctes job/document, ce qui évite de gonfler le résultat quand plusieurs relations relient un document à une personne.

**Une liaison bibliographique n'est pas une preuve historique.** Les événements canoniques et les claims restent inchangés.

La fenêtre est bornée entre 1 et 90 jours ; l'interface expose 7, 30 et 90 jours. La pagination limite les réponses à 50 jobs, avec offset maximum 5 000. Sur de gros volumes, remplacer ensuite l'offset par une pagination keyset sans supprimer les limites.

## Schéma et index

Migration `047_ingestion_admin_indexes.sql` :
- `(created_at DESC,id DESC)` pour l'historique récent ;
- `(lane,status,created_at DESC,id DESC)` pour les filtres explicites.

Les tentatives utilisent toujours la clé primaire `(job_id,source_kind,attempt)` de la migration 046. Les calculs P95 sur une fenêtre temporelle nécessitent une agrégation : surveiller `EXPLAIN (ANALYZE, BUFFERS)` et les temps de réponse avant d'introduire des agrégats matérialisés.

## Contrôles

CI : tests SQLx `admin_ingest`, garde d'accès `routes::admin_ingest`, tests API, Clippy, React, build Vite. Vérifier ensuite le déploiement réel : migrations 044–047 appliquées, variable allowlist côté Render, et contrôle 401/403/200 depuis trois sessions différentes.

Aucune mesure n'est reconstruite rétrospectivement pour les jobs antérieurs à la migration 046.
