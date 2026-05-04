use std::fs;
use tempfile::TempDir;

fn setup(dir: &TempDir) -> (bm25_cli::index::BM25Index, tantivy::Index) {
    let idx = bm25_cli::index::BM25Index::open_at(dir.path()).unwrap();
    let tantivy_index = idx.open_or_create_tantivy().unwrap();
    (idx, tantivy_index)
}

fn index_and_search(files_dir: &std::path::Path, index_dir: &TempDir, query: &str) -> Vec<String> {
    index_and_search_fuzzy(files_dir, index_dir, query, None)
}

fn index_and_search_fuzzy(
    files_dir: &std::path::Path,
    index_dir: &TempDir,
    query: &str,
    fuzzy: Option<u8>,
) -> Vec<String> {
    let (idx, tantivy_index) = setup(index_dir);
    let mut writer = tantivy_index.writer(15_000_000).unwrap();
    let mut mtimes = idx.load_mtimes().unwrap();
    let changed =
        bm25_cli::index_directory(&idx, &tantivy_index, &mut writer, &mut mtimes, files_dir)
            .unwrap();
    if changed {
        writer.commit().unwrap();
        idx.save_mtimes(&mtimes).unwrap();
    }
    drop(writer);
    bm25_cli::search_query_fuzzy(&idx, &tantivy_index, query, files_dir, 50, fuzzy).unwrap()
}

#[test]
fn indexes_files_in_directory() {
    let files = TempDir::new().unwrap();
    let index = TempDir::new().unwrap();

    fs::write(
        files.path().join("payments.md"),
        "PaymentHandler processes refunds",
    )
    .unwrap();
    fs::write(files.path().join("auth.rs"), "fn verify_token() {}").unwrap();

    let results = index_and_search(files.path(), &index, "payment");
    assert!(results.iter().any(|p| p.contains("payments.md")));
    assert!(!results.iter().any(|p| p.contains("auth.rs")));
}

#[test]
fn respects_gitignore() {
    let files = TempDir::new().unwrap();
    let index = TempDir::new().unwrap();

    fs::write(files.path().join(".gitignore"), "secret.txt\n").unwrap();
    fs::write(files.path().join("secret.txt"), "top secret payment info").unwrap();
    fs::write(files.path().join("public.md"), "public payment docs").unwrap();

    let results = index_and_search(files.path(), &index, "payment");
    assert!(
        results.iter().any(|p| p.contains("public.md")),
        "public.md should be indexed"
    );
    assert!(
        !results.iter().any(|p| p.contains("secret.txt")),
        "secret.txt should be excluded by .gitignore"
    );
}

#[test]
fn skips_target_and_node_modules() {
    let files = TempDir::new().unwrap();
    let index = TempDir::new().unwrap();

    fs::create_dir(files.path().join("target")).unwrap();
    fs::write(
        files.path().join("target").join("bloat.rs"),
        "payment bloat",
    )
    .unwrap();
    fs::create_dir(files.path().join("node_modules")).unwrap();
    fs::write(
        files.path().join("node_modules").join("lib.js"),
        "payment lib",
    )
    .unwrap();
    fs::write(files.path().join("main.rs"), "fn main() {}").unwrap();

    let results = index_and_search(files.path(), &index, "payment");
    assert!(!results.iter().any(|p| p.contains("target")));
    assert!(!results.iter().any(|p| p.contains("node_modules")));
}

#[test]
fn removes_deleted_files_on_reindex() {
    let files = TempDir::new().unwrap();
    let index = TempDir::new().unwrap();

    let file_path = files.path().join("payments.md");
    fs::write(&file_path, "PaymentHandler processes refunds").unwrap();

    index_and_search(files.path(), &index, "payment");

    fs::remove_file(&file_path).unwrap();

    let results = index_and_search(files.path(), &index, "payment");
    assert!(
        results.is_empty(),
        "deleted file should not appear in results"
    );
}

#[test]
fn reindexes_modified_files() {
    let files = TempDir::new().unwrap();
    let index = TempDir::new().unwrap();

    let file_path = files.path().join("notes.md");
    fs::write(&file_path, "nothing relevant here").unwrap();

    let results = index_and_search(files.path(), &index, "payment");
    assert!(results.is_empty());

    std::thread::sleep(std::time::Duration::from_secs(1));
    fs::write(&file_path, "PaymentHandler is now here").unwrap();

    let results = index_and_search(files.path(), &index, "payment");
    assert!(results.iter().any(|p| p.contains("notes.md")));
}

#[test]
fn fuzzy_matches_typo() {
    let files = TempDir::new().unwrap();
    let index = TempDir::new().unwrap();

    fs::write(
        files.path().join("payments.md"),
        "PaymentHandler processes refunds",
    )
    .unwrap();

    let exact = index_and_search(files.path(), &index, "paymnet");
    assert!(exact.is_empty(), "exact search should not match a typo");

    let fuzzy = index_and_search_fuzzy(files.path(), &index, "paymnet", Some(1));
    assert!(
        fuzzy.iter().any(|p| p.contains("payments.md")),
        "fuzzy search should match despite typo"
    );
}
