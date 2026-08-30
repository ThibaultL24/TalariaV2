// crates/talaria-store/src/claims.rs
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ClaimRow {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub claim_kind: String,
    pub text: String,
    pub epistemic_status: String,
    pub relation_to_subject: String,
    pub event_time: Option<chrono::DateTime<chrono::Utc>>,
    pub place_label: Option<String>,
    pub confidence: f64,
    pub canonical_event_id: Option<Uuid>,
    pub debate_type: Option<String>,
    pub evidence_layer: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ClaimInsert {
    pub entity_id: Uuid,
    pub claim_kind: String,
    pub text: String,
    pub epistemic_status: String,
    pub relation_to_subject: String,
    pub event_time: Option<chrono::DateTime<chrono::Utc>>,
    pub place_label: Option<String>,
    pub confidence: f64,
    pub canonical_event_id: Option<Uuid>,
    pub debate_type: Option<String>,
    pub evidence_layer: Option<String>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ClaimEvidenceRow {
    pub id: Uuid,
    pub claim_id: Uuid,
    pub source_system: String,
    pub locator: Option<String>,
    pub quote: Option<String>,
    pub sentence_id: Option<Uuid>,
    pub confidence: f64,
}

pub async fn insert_claim(pool: &PgPool, claim: &ClaimInsert) -> anyhow::Result<Uuid> {
    let id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO soft_claims (
            entity_id, claim_kind, text, epistemic_status, relation_to_subject,
            event_time, place_label, confidence, canonical_event_id,
            debate_type, evidence_layer
        )
        VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)
        RETURNING id
        "#,
    )
    .bind(claim.entity_id)
    .bind(&claim.claim_kind)
    .bind(&claim.text)
    .bind(&claim.epistemic_status)
    .bind(&claim.relation_to_subject)
    .bind(claim.event_time)
    .bind(&claim.place_label)
    .bind(claim.confidence)
    .bind(claim.canonical_event_id)
    .bind(&claim.debate_type)
    .bind(&claim.evidence_layer)
    .fetch_one(pool)
    .await?;
    Ok(id)
}

pub async fn find_claim_by_text(
    pool: &PgPool,
    entity_id: Uuid,
    text: &str,
) -> anyhow::Result<Option<Uuid>> {
    let id: Option<Uuid> = sqlx::query_scalar(
        r#"
        SELECT id FROM soft_claims
        WHERE entity_id = $1 AND text = $2
        LIMIT 1
        "#,
    )
    .bind(entity_id)
    .bind(text)
    .fetch_optional(pool)
    .await?;
    Ok(id)
}

pub async fn insert_claim_evidence(
    pool: &PgPool,
    claim_id: Uuid,
    source_system: &str,
    locator: Option<&str>,
    quote: Option<&str>,
    sentence_id: Option<Uuid>,
    confidence: f64,
) -> anyhow::Result<Uuid> {
    let id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO soft_claim_evidence (
            claim_id, source_system, locator, quote, sentence_id, confidence
        )
        VALUES ($1,$2,$3,$4,$5,$6)
        RETURNING id
        "#,
    )
    .bind(claim_id)
    .bind(source_system)
    .bind(locator)
    .bind(quote)
    .bind(sentence_id)
    .bind(confidence)
    .fetch_one(pool)
    .await?;
    Ok(id)
}

pub async fn insert_claim_relation(
    pool: &PgPool,
    from_claim_id: Uuid,
    to_claim_id: Uuid,
    relation: &str,
) -> anyhow::Result<Uuid> {
    let id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO soft_claim_relations (from_claim_id, to_claim_id, relation)
        VALUES ($1,$2,$3)
        ON CONFLICT (from_claim_id, to_claim_id, relation) DO UPDATE
          SET relation = EXCLUDED.relation
        RETURNING id
        "#,
    )
    .bind(from_claim_id)
    .bind(to_claim_id)
    .bind(relation)
    .fetch_one(pool)
    .await?;
    Ok(id)
}

pub async fn list_claims_for_entity(
    pool: &PgPool,
    entity_id: Uuid,
    limit: i64,
    debates_only: bool,
) -> anyhow::Result<Vec<ClaimRow>> {
    let rows = sqlx::query_as::<_, ClaimRow>(
        r#"
        SELECT id, entity_id, claim_kind, text, epistemic_status, relation_to_subject,
               event_time, place_label, confidence, canonical_event_id,
               debate_type, evidence_layer
        FROM soft_claims
        WHERE entity_id IN (
            SELECT e2.id
            FROM entities e1
            JOIN entities e2 ON e2.id = e1.id
               OR (e1.qid IS NOT NULL AND e2.qid = e1.qid)
            WHERE e1.id = $1
          )
          AND (
            NOT $3
            OR claim_kind IN ('theory', 'controversy', 'debate_stance')
            OR relation_to_subject = 'historiography'
          )
        ORDER BY confidence DESC, created_at ASC
        LIMIT $2
        "#,
    )
    .bind(entity_id)
    .bind(limit)
    .bind(debates_only)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn list_claim_evidence(
    pool: &PgPool,
    claim_id: Uuid,
) -> anyhow::Result<Vec<ClaimEvidenceRow>> {
    let rows = sqlx::query_as::<_, ClaimEvidenceRow>(
        r#"
        SELECT id, claim_id, source_system, locator, quote, sentence_id, confidence
        FROM soft_claim_evidence
        WHERE claim_id = $1
        ORDER BY confidence DESC
        "#,
    )
    .bind(claim_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SentenceForClaims {
    pub id: Uuid,
    pub text: String,
    pub ordinal: i32,
    pub page_title: String,
    pub wiki_lang: String,
    pub revision_id: Option<i64>,
    pub entity_id: Option<Uuid>,
}

/// Sentences joined to an entity via matching wikipedia title.
pub async fn list_sentences_for_claims(
    pool: &PgPool,
    limit: i64,
) -> anyhow::Result<Vec<SentenceForClaims>> {
    let rows = sqlx::query_as::<_, SentenceForClaims>(
        r#"
        SELECT
            s.id,
            s.text,
            s.ordinal,
            wp.title AS page_title,
            wp.wiki_lang,
            wp.revision_id,
            COALESCE(
              e.id,
              (
                SELECT pc.entity_id
                FROM phrase_candidates pc
                WHERE pc.sentence_id = s.id AND pc.entity_id IS NOT NULL
                LIMIT 1
              )
            ) AS entity_id
        FROM sentences s
        INNER JOIN wiki_pages wp ON wp.id = s.wiki_page_id
        LEFT JOIN entities e
          ON e.wiki_lang = wp.wiki_lang
         AND e.wikipedia_title = wp.title
        WHERE (
            e.id IS NOT NULL
            OR EXISTS (
              SELECT 1 FROM phrase_candidates pc
              WHERE pc.sentence_id = s.id AND pc.entity_id IS NOT NULL
            )
          )
          AND NOT EXISTS (
            SELECT 1 FROM soft_claim_evidence ce WHERE ce.sentence_id = s.id
          )
        ORDER BY wp.title ASC, s.ordinal ASC
        LIMIT $1
        "#,
    )
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn backfill_life_event_claims(pool: &PgPool) -> anyhow::Result<usize> {
    let rows = sqlx::query_as::<
        _,
        (
            Uuid,
            Uuid,
            String,
            String,
            Option<chrono::DateTime<chrono::Utc>>,
            Option<String>,
            f64,
        ),
    >(
        r#"
        SELECT ce.id, ce.entity_id,
               COALESCE(ce.summary, ce.title) AS text,
               ce.event_type,
               ce.start_time, ce.place_label, ce.confidence
        FROM canonical_events ce
        WHERE NOT EXISTS (
          SELECT 1 FROM soft_claims c WHERE c.canonical_event_id = ce.id
        )
        "#,
    )
    .fetch_all(pool)
    .await?;

    let mut n = 0;
    for (event_id, entity_id, text, event_type, event_time, place_label, confidence) in rows {
        let claim_id = insert_claim(
            pool,
            &ClaimInsert {
                entity_id,
                claim_kind: claim_kind_for_event(&event_type).into(),
                text: text.clone(),
                epistemic_status: "attested".into(),
                relation_to_subject: "direct".into(),
                event_time,
                place_label,
                confidence,
                canonical_event_id: Some(event_id),
                debate_type: None,
                evidence_layer: None,
            },
        )
        .await?;

        if let Some((quote, sentence_id, wiki_lang, wiki_title, revision_id)) = sqlx::query_as::<
            _,
            (
                Option<String>,
                Option<Uuid>,
                Option<String>,
                Option<String>,
                Option<i64>,
            ),
        >(
            r#"
            SELECT
              COALESCE(ee.quoted_text, s.text),
              ee.sentence_id,
              wp.wiki_lang,
              wp.title,
              wp.revision_id
            FROM event_evidence ee
            LEFT JOIN sentences s ON s.id = ee.sentence_id
            LEFT JOIN wiki_pages wp ON wp.id = s.wiki_page_id
            WHERE ee.canonical_event_id = $1
            ORDER BY ee.confidence DESC NULLS LAST
            LIMIT 1
            "#,
        )
        .bind(event_id)
        .fetch_optional(pool)
        .await?
        {
            let locator = match (wiki_lang.as_deref(), wiki_title.as_deref(), revision_id) {
                (Some(lang), Some(title), Some(oldid)) => Some(format!(
                    "https://{lang}.wikipedia.org/w/index.php?title={}&oldid={oldid}",
                    title.replace(' ', "_")
                )),
                (Some(lang), Some(title), None) => Some(format!(
                    "https://{lang}.wikipedia.org/wiki/{}",
                    title.replace(' ', "_")
                )),
                _ => None,
            };
            insert_claim_evidence(
                pool,
                claim_id,
                "wikipedia",
                locator.as_deref(),
                quote.as_deref(),
                sentence_id,
                confidence,
            )
            .await?;
        }
        n += 1;
    }
    Ok(n)
}

fn claim_kind_for_event(event_type: &str) -> &'static str {
    match event_type {
        "anecdote" => "anecdote",
        "statue" | "museum" | "memorial" | "street_naming" => "fact",
        _ => "life_event",
    }
}
