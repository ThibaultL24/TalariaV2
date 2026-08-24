// crates/talaria-store/src/wikibase.rs
//! Persist normalized Wikibase statements (full claims, not TSV STATEMENT lines).

use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct WikibaseStatementInsert {
    pub qid: String,
    pub guid: String,
    pub property: String,
    pub rank: String,
    pub snaktype: String,
    pub value_json: serde_json::Value,
    pub qualifiers_json: serde_json::Value,
    pub references_json: serde_json::Value,
    pub revision_id: Option<String>,
}

pub async fn upsert_wikibase_statement(
    pool: &PgPool,
    row: &WikibaseStatementInsert,
) -> anyhow::Result<Uuid> {
    let id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO wikibase_statements (
            qid, guid, property, rank, snaktype,
            value_json, qualifiers_json, references_json, revision_id
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        ON CONFLICT (guid, revision_id) DO UPDATE SET
            qid = EXCLUDED.qid,
            property = EXCLUDED.property,
            rank = EXCLUDED.rank,
            snaktype = EXCLUDED.snaktype,
            value_json = EXCLUDED.value_json,
            qualifiers_json = EXCLUDED.qualifiers_json,
            references_json = EXCLUDED.references_json
        RETURNING id
        "#,
    )
    .bind(&row.qid)
    .bind(&row.guid)
    .bind(&row.property)
    .bind(&row.rank)
    .bind(&row.snaktype)
    .bind(&row.value_json)
    .bind(&row.qualifiers_json)
    .bind(&row.references_json)
    .bind(&row.revision_id)
    .fetch_one(pool)
    .await?;

    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migration_024_creates_wikibase_statements() {
        let sql = include_str!("../../../migrations/024_wikibase_statements.sql");
        assert!(sql.contains("CREATE TABLE IF NOT EXISTS wikibase_statements"));
        assert!(sql.contains("UNIQUE (guid, revision_id)"));
        assert!(sql.contains("idx_wikibase_statements_qid"));
        assert!(sql.contains("idx_wikibase_statements_pid"));
        assert!(sql.contains("qualifiers_json JSONB NOT NULL DEFAULT '{}'::jsonb"));
        assert!(sql.contains("references_json JSONB NOT NULL DEFAULT '[]'::jsonb"));
    }

    #[test]
    fn upsert_conflicts_on_guid_and_revision_id() {
        let src = include_str!("wikibase.rs");
        let prod = src.split("#[cfg(test)]").next().expect("prod source");
        assert!(prod.contains("ON CONFLICT (guid, revision_id) DO UPDATE SET"));
        assert!(prod.contains("RETURNING id"));
    }

    #[test]
    fn statement_insert_carries_json_payloads() {
        let row = WikibaseStatementInsert {
            qid: "Q517".into(),
            guid: "Q517$abc".into(),
            property: "P569".into(),
            rank: "normal".into(),
            snaktype: "value".into(),
            value_json: serde_json::json!({"time": "+1769-08-15T00:00:00Z"}),
            qualifiers_json: serde_json::json!({}),
            references_json: serde_json::json!([]),
            revision_id: Some("123".into()),
        };
        assert_eq!(row.qid, "Q517");
        assert_eq!(row.property, "P569");
        assert_eq!(row.value_json["time"], "+1769-08-15T00:00:00Z");
        assert_eq!(row.revision_id.as_deref(), Some("123"));
    }
}
