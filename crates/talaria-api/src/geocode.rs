// crates/talaria-api/src/geocode.rs
use talaria_core::AppConfig;
use talaria_store::{
    apply_geocode_to_events, connect, get_place_geocode, list_place_labels_needing_geocode,
    run_migrations, upsert_place_geocode,
};
use talaria_wikidata::{geocode_place_label, WikidataClient};

pub async fn run_geocode_places(config: &AppConfig, limit: i64) -> anyhow::Result<()> {
    let pool = connect(config).await?;
    run_migrations(&pool).await?;

    let label_limit = if limit > 0 { limit } else { i64::MAX };
    let labels = list_place_labels_needing_geocode(&pool, &config.wiki_lang, label_limit).await?;

    tracing::info!(labels = labels.len(), "geocoding place labels via Wikidata");

    let client = WikidataClient::new()?;
    let mut geocoded = 0usize;
    let mut events_updated = 0usize;
    let mut failed = 0usize;

    for label in labels {
        if get_place_geocode(&pool, &config.wiki_lang, &label)
            .await?
            .is_some()
        {
            continue;
        }

        let result = geocode_place_label(&client, &label, &config.wiki_lang).await;
        match result {
            Ok(Some(place)) => {
                upsert_place_geocode(
                    &pool,
                    &config.wiki_lang,
                    &label,
                    &place.wikidata_qid,
                    place.lat,
                    place.lon,
                    place.raw_json,
                )
                .await?;

                let updated =
                    apply_geocode_to_events(&pool, &config.wiki_lang, &label, place.lat, place.lon)
                        .await?;
                geocoded += 1;
                events_updated += updated as usize;

                tracing::info!(
                    label = %label,
                    qid = %place.wikidata_qid,
                    lat = place.lat,
                    lon = place.lon,
                    updated,
                    "place geocoded"
                );
            }
            Ok(None) => {
                failed += 1;
                tracing::warn!(label = %label, "no wikidata coordinates found");
            }
            Err(err) => {
                failed += 1;
                tracing::warn!(label = %label, %err, "wikidata geocode failed");
            }
        }

        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }

    tracing::info!(geocoded, events_updated, failed, "geocoding complete");
    Ok(())
}
