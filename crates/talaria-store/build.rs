// crates/talaria-store/build.rs
// sqlx::migrate! embeds files at compile time. Without this, adding
// migrations/040_*.sql does not rebuild talaria-store, so serve sees a
// migrator missing version 40 while _sqlx_migrations already has it.
fn main() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../migrations");
    println!("cargo:rerun-if-changed={}", dir.display());
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            println!("cargo:rerun-if-changed={}", entry.path().display());
        }
    }
}
