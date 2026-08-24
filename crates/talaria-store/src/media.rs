// crates/talaria-store/src/media.rs
//! Persist attributed Commons media records (thumbs + license; never auto-events).

use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct MediaAssetInsert {
    pub commons_file: String,
    pub mid: Option<String>,
    pub sha1: Option<String>,
    pub mime: Option<String>,
    pub license: Option<String>,
    pub attribution_text: String,
    pub thumb_url: Option<String>,
    pub depicts_qids: Vec<String>,
    pub revision_id: Option<String>,
    pub rights_normalized: String,
    pub entity_id: Option<Uuid>,
    pub corpus_document_id: Option<Uuid>,
}

pub async fn upsert_media_asset(pool: &PgPool, row: &MediaAssetInsert) -> anyhow::Result<Uuid> {
    let sql = if row.mid.is_some() {
        r#"
        INSERT INTO media_assets (
            commons_file, mid, sha1, mime, license, attribution_text,
            thumb_url, depicts_qids, revision_id, rights_normalized,
            entity_id, corpus_document_id
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
        ON CONFLICT (mid) WHERE mid IS NOT NULL DO UPDATE SET
            commons_file = EXCLUDED.commons_file,
            sha1 = EXCLUDED.sha1,
            mime = EXCLUDED.mime,
            license = EXCLUDED.license,
            attribution_text = EXCLUDED.attribution_text,
            thumb_url = EXCLUDED.thumb_url,
            depicts_qids = EXCLUDED.depicts_qids,
            revision_id = EXCLUDED.revision_id,
            rights_normalized = EXCLUDED.rights_normalized,
            entity_id = EXCLUDED.entity_id,
            corpus_document_id = EXCLUDED.corpus_document_id
        RETURNING id
        "#
    } else {
        r#"
        INSERT INTO media_assets (
            commons_file, mid, sha1, mime, license, attribution_text,
            thumb_url, depicts_qids, revision_id, rights_normalized,
            entity_id, corpus_document_id
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
        ON CONFLICT (commons_file, sha1) DO UPDATE SET
            mid = EXCLUDED.mid,
            mime = EXCLUDED.mime,
            license = EXCLUDED.license,
            attribution_text = EXCLUDED.attribution_text,
            thumb_url = EXCLUDED.thumb_url,
            depicts_qids = EXCLUDED.depicts_qids,
            revision_id = EXCLUDED.revision_id,
            rights_normalized = EXCLUDED.rights_normalized,
            entity_id = EXCLUDED.entity_id,
            corpus_document_id = EXCLUDED.corpus_document_id
        RETURNING id
        "#
    };

    let id: Uuid = sqlx::query_scalar(sql)
        .bind(&row.commons_file)
        .bind(&row.mid)
        .bind(&row.sha1)
        .bind(&row.mime)
        .bind(&row.license)
        .bind(&row.attribution_text)
        .bind(&row.thumb_url)
        .bind(&row.depicts_qids)
        .bind(&row.revision_id)
        .bind(&row.rights_normalized)
        .bind(&row.entity_id)
        .bind(&row.corpus_document_id)
        .fetch_one(pool)
        .await?;

    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migration_025_creates_media_assets() {
        let sql = include_str!("../../../migrations/025_media_assets.sql");
        assert!(sql.contains("CREATE TABLE IF NOT EXISTS media_assets"));
        assert!(sql.contains("UNIQUE NULLS NOT DISTINCT (commons_file, sha1)"));
        assert!(sql.contains("idx_media_assets_mid"));
        assert!(sql.contains("WHERE mid IS NOT NULL"));
        assert!(sql.contains("depicts_qids TEXT[] NOT NULL DEFAULT '{}'"));
        assert!(sql.contains("attribution_text TEXT NOT NULL"));
        assert!(sql.contains("REFERENCES entities(id) ON DELETE SET NULL"));
        assert!(sql.contains("REFERENCES corpus_documents(id) ON DELETE SET NULL"));
    }

    #[test]
    fn upsert_conflicts_on_mid_or_commons_file_and_sha1() {
        let src = include_str!("media.rs");
        let prod = src.split("#[cfg(test)]").next().expect("prod source");
        assert!(prod.contains("ON CONFLICT (mid) WHERE mid IS NOT NULL DO UPDATE SET"));
        assert!(prod.contains("ON CONFLICT (commons_file, sha1) DO UPDATE SET"));
        assert!(prod.contains("RETURNING id"));
    }

    #[test]
    fn media_asset_insert_requires_attribution_text() {
        let row = MediaAssetInsert {
            commons_file: "File:Napoleon_crossing_the_Alps.jpg".into(),
            mid: Some("M12345".into()),
            sha1: Some("abc123".into()),
            mime: Some("image/jpeg".into()),
            license: Some("CC BY-SA 4.0".into()),
            attribution_text: "Author / Wikimedia Commons".into(),
            thumb_url: Some("https://upload.wikimedia.org/.../thumb.jpg".into()),
            depicts_qids: vec!["Q517".into()],
            revision_id: Some("987654".into()),
            rights_normalized: "cc-by-sa-4.0".into(),
            entity_id: None,
            corpus_document_id: None,
        };
        assert_eq!(row.commons_file, "File:Napoleon_crossing_the_Alps.jpg");
        assert_eq!(row.attribution_text, "Author / Wikimedia Commons");
        assert_eq!(row.depicts_qids, vec!["Q517"]);
        assert_eq!(row.rights_normalized, "cc-by-sa-4.0");
    }
}
