-- Poreia pilot import. Staging only. Every row stays in quarantine.
-- Do not point this file at a production database.
-- Re-running it must not publish anything and must not edit Istoria history rows.

INSERT INTO visit_sites (
    wikidata_qid, label, kind, geom, publication_status, visitability, retrieved_at
)
VALUES
    (
        'Q59210966',
        'Tombeau de l''Empereur Napoléon aux Invalides',
        'burial_site',
        ST_SetSRID(ST_MakePoint(2.312514, 48.855242), 4326)::geography,
        'quarantine',
        'unknown',
        TIMESTAMPTZ '2026-11-08 22:00:00+00'
    ),
    (
        'Q64436',
        'arc de triomphe de l''Étoile',
        'monument',
        ST_SetSRID(ST_MakePoint(2.29504, 48.87378), 4326)::geography,
        'quarantine',
        'unknown',
        TIMESTAMPTZ '2026-11-08 22:00:00+00'
    ),
    (
        'Q315318',
        'pont Van-Gogh',
        'bridge',
        ST_SetSRID(ST_MakePoint(4.62111111, 43.65694444), 4326)::geography,
        'quarantine',
        'unknown',
        TIMESTAMPTZ '2026-11-08 22:00:00+00'
    ),
    (
        'Q17076145',
        'Fondation Vincent van Gogh Arles',
        'cultural_institution',
        ST_SetSRID(ST_MakePoint(4.625651, 43.678131), 4326)::geography,
        'quarantine',
        'unknown',
        TIMESTAMPTZ '2026-11-08 22:00:00+00'
    )
ON CONFLICT (wikidata_qid) DO UPDATE SET
    label = EXCLUDED.label,
    kind = EXCLUDED.kind,
    geom = EXCLUDED.geom,
    visitability = EXCLUDED.visitability,
    retrieved_at = EXCLUDED.retrieved_at,
    updated_at = NOW();

INSERT INTO visit_site_narratives (
    site_id, subject_qid, relation_role, relation_pid, summary, source_url,
    retrieved_at, publication_status
)
SELECT site.id, narrative.subject_qid, narrative.relation_role, narrative.relation_pid,
       narrative.summary, narrative.source_url,
       TIMESTAMPTZ '2026-11-08 22:00:00+00', 'quarantine'
FROM (VALUES
    (
        'Q59210966', 'Q517', 'thematic', NULL::text,
        $summary$Wikidata nomme ce monument « Tombeau de l'Empereur Napoléon aux Invalides » et le situe dans le dôme des Invalides (P276 = Q152109). La commande portée par la fiche est celle de Louis-Philippe Ier (P88 = Q7771). Les énoncés P119 de Napoléon (Q517) désignent la cathédrale Saint-Louis-des-Invalides (Q4992220) et la Vallée du Tombeau (Q3554092), pas cet identifiant. Le lien retenu est l'intitulé de la fiche, pas une sépulture P119.$summary$,
        'https://www.wikidata.org/wiki/Q59210966'
    ),
    (
        'Q64436', 'Q517', 'commissioned', 'P88',
        $summary$Wikidata indique que l'arc de triomphe de l'Étoile a été commandé par Napoléon (P88 = Q517). La fiche ne porte pas d'énoncé P547 (commémore). Coordonnées P625 : 48.87378, 2.29504.$summary$,
        'https://www.wikidata.org/wiki/Q64436'
    ),
    (
        'Q315318', 'Q5582', 'named_after', 'P138',
        $summary$Wikidata indique que ce pont est nommé d'après Vincent van Gogh (P138 = Q5582). La fiche ne porte pas de résidence P551. Coordonnées P625 : 43.65694444, 4.62111111.$summary$,
        'https://www.wikidata.org/wiki/Q315318'
    ),
    (
        'Q17076145', 'Q5582', 'thematic', NULL::text,
        $summary$Wikidata date la création de la Fondation Vincent van Gogh Arles de 1985, à la précision de l'année (P571), et la situe à Arles (P276 = Q48292). Vincent van Gogh est mort en 1890 : cette fiche ne documente ni une résidence ni une visite. Le lien est thématique.$summary$,
        'https://www.wikidata.org/wiki/Q17076145'
    )
) AS narrative(site_qid, subject_qid, relation_role, relation_pid, summary, source_url)
JOIN visit_sites site ON site.wikidata_qid = narrative.site_qid
ON CONFLICT (site_id, subject_qid, relation_role, source_url) DO UPDATE SET
    summary = EXCLUDED.summary,
    relation_pid = EXCLUDED.relation_pid,
    retrieved_at = EXCLUDED.retrieved_at;

INSERT INTO visit_site_media (
    site_id, commons_title, author, license, attribution, source_url,
    publication_status, retrieved_at
)
SELECT site.id,
       'Arc de Triomphe - Août 2026.jpg',
       'ZeusUpsistos',
       'CC BY-SA 4.0',
       'Own work',
       'https://commons.wikimedia.org/wiki/File:Arc_de_Triomphe_-_Ao%C3%BBt_2026.jpg',
       'quarantine',
       TIMESTAMPTZ '2026-11-08 22:00:00+00'
FROM visit_sites site
WHERE site.wikidata_qid = 'Q64436'
ON CONFLICT (site_id, source_url) DO UPDATE SET
    commons_title = EXCLUDED.commons_title,
    author = EXCLUDED.author,
    license = EXCLUDED.license,
    attribution = EXCLUDED.attribution,
    retrieved_at = EXCLUDED.retrieved_at;

INSERT INTO poreia_occurrences (
    site_id, kind, title, summary, venue_label, geom, occurs_on, ends_on,
    time_precision, source_url, source_record_id, publication_status, fetched_at
)
SELECT site.id,
       'exhibition',
       'Van Gogh et les Étoiles',
       $summary$Wikidata situe l'exposition « Van Gogh et les Étoiles » à la Fondation Vincent van Gogh Arles (P276 = Q17076145), du 2024-06-01 au 2024-09-08, précision du jour. Les énoncés consultés le 2026-11-08 n'ont pas de référence externe. Aucune heure n'est connue.$summary$,
       'Fondation Vincent van Gogh Arles',
       ST_SetSRID(ST_MakePoint(4.625651, 43.678131), 4326)::geography,
       DATE '2024-06-01',
       DATE '2024-09-08',
       'day',
       'https://www.wikidata.org/wiki/Q141613312',
       'Q141613312',
       'quarantine',
       TIMESTAMPTZ '2026-11-08 22:00:00+00'
FROM visit_sites site
WHERE site.wikidata_qid = 'Q17076145'
ON CONFLICT (source_url, source_record_id) DO UPDATE SET
    title = EXCLUDED.title,
    summary = EXCLUDED.summary,
    venue_label = EXCLUDED.venue_label,
    geom = EXCLUDED.geom,
    occurs_on = EXCLUDED.occurs_on,
    ends_on = EXCLUDED.ends_on,
    time_precision = EXCLUDED.time_precision,
    fetched_at = EXCLUDED.fetched_at;
