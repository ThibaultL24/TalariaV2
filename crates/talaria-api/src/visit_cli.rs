// crates/talaria-api/src/visit_cli.rs
//! Visit operator commands (local Mérimée import).

use std::path::PathBuf;
use std::time::Instant;
use talaria_core::AppConfig;
use talaria_store::{
    catalog_heritage, collect_audit_snapshot, connect, count_notices, dry_run_csv, import_merimee_csv,
    import_merimee_file, merimee_hits_for_entity, resume_row_from_last_partial, run_migrations, DEFAULT_CSV_URL,
};

pub async fn run_import_merimee(
    config: &AppConfig,
    file: Option<PathBuf>,
    url: Option<String>,
    resume_after_row: u64,
    resume_last_partial: bool,
    skip_dry_run: bool,
) -> anyhow::Result<()> {
    let pool = connect(config).await?;
    run_migrations(&pool).await?;
    let source_uri = url.clone().unwrap_or_else(|| {
        file.as_ref()
            .map(|path| format!("file://{}", path.display()))
            .unwrap_or_else(|| DEFAULT_CSV_URL.to_string())
    });
    let resume_after_row = if resume_last_partial {
        let from_db = resume_row_from_last_partial(&pool, &source_uri).await?;
        println!("phase=resume resolved_row={from_db} source={source_uri}");
        from_db
    } else {
        resume_after_row
    };

    let report = if let Some(path) = file {
        let bytes = std::fs::read(&path)?;
        println!(
            "phase=read_file path={} bytes={} elapsed_ms={}",
            path.display(),
            bytes.len(),
            0
        );
        if !skip_dry_run {
            let dry = dry_run_csv(bytes.as_slice())?;
            print_dry_run(&dry);
        }
        let checksum = sha256_hex(&bytes);
        let started = Instant::now();
        println!("phase=import_persist start resume_after_row={resume_after_row}");
        let reader = std::io::Cursor::new(bytes);
        let report = import_merimee_csv(
            &pool,
            &source_uri,
            Some(&checksum),
            reader,
            resume_after_row,
        )
        .await?;
        println!(
            "phase=import_persist done elapsed_ms={}",
            started.elapsed().as_millis()
        );
        println!("phase=file_checksum sha256={checksum}");
        report
    } else {
        let download_url = url.unwrap_or_else(|| DEFAULT_CSV_URL.to_string());
        let started = Instant::now();
        println!("phase=download start url={download_url}");
        let client = reqwest::Client::builder()
            .user_agent("TalariaEngine/0.1 (merimee import)")
            .timeout(std::time::Duration::from_secs(600))
            .build()?;
        let bytes = client.get(&download_url).send().await?.error_for_status()?.bytes().await?;
        println!(
            "phase=download done bytes={} elapsed_ms={}",
            bytes.len(),
            started.elapsed().as_millis()
        );
        if !skip_dry_run {
            let dry = dry_run_csv(bytes.as_ref())?;
            print_dry_run(&dry);
        }
        let checksum = sha256_hex(&bytes);
        let reader = std::io::Cursor::new(bytes.to_vec());
        let persist_started = Instant::now();
        println!("phase=import_persist start resume_after_row={resume_after_row}");
        let report = import_merimee_csv(
            &pool,
            &download_url,
            Some(&checksum),
            reader,
            resume_after_row,
        )
        .await?;
        println!(
            "phase=import_persist done elapsed_ms={}",
            persist_started.elapsed().as_millis()
        );
        println!("phase=file_checksum sha256={checksum}");
        report
    };

    println!(
        "phase=import_result run_id={} seen={} upserted={} unchanged={} rejected={} entity_links={} heritage_links={}",
        report.run_id,
        report.rows_seen,
        report.rows_upserted,
        report.rows_unchanged,
        report.rows_rejected,
        report.entity_links_added,
        report.heritage_links_added,
    );
    let total = count_notices(&pool).await?;
    println!("merimee_notices_total={total}");
    let audit = collect_audit_snapshot(&pool).await?;
    println!("phase=audit {}", serde_json::to_string(&audit)?);
    Ok(())
}

fn print_dry_run(dry: &talaria_store::CsvParseDryRun) {
    println!(
        "phase=parse_dry_run accepted={} rejected={} reasons={}",
        dry.rows_accepted,
        dry.rows_rejected,
        serde_json::to_string(&dry.reject_reasons).unwrap_or_else(|_| "{}".into()),
    );
}

pub async fn run_merimee_status(config: &AppConfig) -> anyhow::Result<()> {
    let pool = connect(config).await?;
    run_migrations(&pool).await?;
    let audit = collect_audit_snapshot(&pool).await?;
    println!("{}", serde_json::to_string_pretty(&audit)?);
    Ok(())
}

pub async fn run_merimee_coverage(config: &AppConfig, qid: &str, label: &str) -> anyhow::Result<()> {
    let pool = connect(config).await?;
    run_migrations(&pool).await?;
    let hits = merimee_hits_for_entity(&pool, label, Some(qid)).await?;
    let report = catalog_heritage(qid, label, &[], &[], &hits, &[]);
    println!(
        "{label} {qid} merimee_hits={} catalog_accepted={} catalog_rejected={}",
        hits.len(),
        report.accepted.len(),
        report.rejected.len(),
    );
    for site in &report.accepted {
        println!(
            "  + {} ref={} relation={}",
            site.label,
            site.merimee_ref.as_deref().unwrap_or("-"),
            site.relations.first().map(|r| r.relation.as_str()).unwrap_or("-"),
        );
    }
    for reject in &report.rejected {
        println!("  - {} ({})", reject.key, reject.reason);
    }
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(bytes))
}
