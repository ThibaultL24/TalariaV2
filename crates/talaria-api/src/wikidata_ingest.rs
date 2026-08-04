// crates/talaria-api/src/wikidata_ingest.rs
//! Offline Wikidata dump → entities.qid + entity_profiles + century links.

use std::path::PathBuf;
use talaria_core::AppConfig;
use talaria_store::{
    connect, link_entity_to_centuries, run_migrations, seed_default_periods,
    upsert_entity_from_wikidata, upsert_entity_profile,
};
use talaria_wikidata::{stream_humans, WikidataHuman};

pub async fn run_wikidata_ingest(
    config: &AppConfig,
    dump: Option<PathBuf>,
    limit: usize,
) -> anyhow::Result<()> {
    talaria_dump::ensure_data_dirs(config)?;
    let pool = connect(config).await?;
    run_migrations(&pool).await?;
    let _ = seed_default_periods(&pool).await?;

    let path = resolve_dump_path(config, dump)?;
    tracing::info!(path = %path.display(), limit, "wikidata ingest start");

    let mut humans = Vec::new();
    let stats = stream_humans(&path, limit, |human| {
        humans.push(human);
        Ok(())
    })?;

    let mut linked = 0usize;
    let mut profiles = 0usize;
    for human in &humans {
        let entity_id = persist_human(&pool, human).await?;
        linked += 1;
        profiles += human.profiles.len();
        tracing::info!(
            qid = %human.qid,
            label = %human.label,
            entity_id = %entity_id,
            profile_count = human.profiles.len(),
            "wikidata human linked"
        );
    }

    tracing::info!(
        entities_seen = stats.entities_seen,
        humans_seen = stats.humans_seen,
        humans_linked = linked,
        profiles_upserted = profiles,
        "wikidata ingest done"
    );
    Ok(())
}

async fn persist_human(
    pool: &sqlx::PgPool,
    human: &WikidataHuman,
) -> anyhow::Result<uuid::Uuid> {
    let entity_id = resolve_entity(pool, human).await?;

    for profile in &human.profiles {
        upsert_entity_profile(
            pool,
            entity_id,
            &profile.slug,
            &profile.label,
            &profile.kind,
            Some(&profile.qid),
            0.9,
            "wikidata-dump",
        )
        .await?;
    }

    let _ = link_entity_to_centuries(pool, entity_id, human.birth_year, human.death_year).await?;
    Ok(entity_id)
}

async fn resolve_entity(
    pool: &sqlx::PgPool,
    human: &WikidataHuman,
) -> anyhow::Result<uuid::Uuid> {
    use talaria_store::{find_entity_by_qid, find_entity_by_wikipedia_title, update_entity_qid};

    if let Some(existing) = find_entity_by_qid(pool, &human.qid).await? {
        return Ok(existing.id);
    }

    // Prefer attaching QID onto an already-local Wikipedia surface.
    for link in &human.sitelinks {
        if let Some(existing) =
            find_entity_by_wikipedia_title(pool, &link.wiki_lang, &link.title).await?
        {
            update_entity_qid(pool, existing.id, &human.qid).await?;
            sqlx::query(
                r#"
                UPDATE entities
                SET canonical_name = COALESCE(canonical_name, $2)
                WHERE id = $1
                "#,
            )
            .bind(existing.id)
            .bind(&human.label)
            .execute(pool)
            .await?;
            return Ok(existing.id);
        }
    }

    let preferred = human
        .sitelinks
        .iter()
        .find(|link| link.wiki_lang == "en")
        .or_else(|| human.sitelinks.first());

    let (wiki_lang, title) = match preferred {
        Some(link) => (link.wiki_lang.as_str(), link.title.as_str()),
        None => ("en", human.label.as_str()),
    };

    upsert_entity_from_wikidata(pool, &human.qid, &human.label, wiki_lang, title).await
}

fn resolve_dump_path(config: &AppConfig, dump: Option<PathBuf>) -> anyhow::Result<PathBuf> {
    if let Some(path) = dump {
        return Ok(path);
    }

    let candidates = [
        config.dumps_dir().join("wikidata-sample-humans.json"),
        config.wikidata_dir().join("wikidata-sample-humans.json"),
        PathBuf::from("fixtures/wikidata/wikidata-sample-humans.json"),
    ];
    for path in candidates {
        if path.exists() {
            return Ok(path);
        }
    }

    if let Ok(entries) = std::fs::read_dir(config.dumps_dir()) {
        let mut matches: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                name.contains("wikidata")
                    && (name.ends_with(".json")
                        || name.ends_with(".json.bz2")
                        || name.ends_with(".json.gz"))
            })
            .collect();
        matches.sort();
        if let Some(path) = matches.into_iter().next() {
            return Ok(path);
        }
    }

    anyhow::bail!(
        "no Wikidata dump found; pass --dump PATH or place file under {}",
        config.dumps_dir().display()
    )
}
