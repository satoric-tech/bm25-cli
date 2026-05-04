use std::fs;
use std::process::Command;
use tempfile::TempDir;

fn bm25_bin() -> std::path::PathBuf {
    std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("bm25")
}

fn cmd(args: &[&str], home_dir: &std::path::Path) -> std::process::Output {
    Command::new(bm25_bin())
        .env("HOME", home_dir)
        .args(args)
        .output()
        .unwrap()
}

fn setup() -> (TempDir, TempDir) {
    let home = TempDir::new().unwrap();
    let files = TempDir::new().unwrap();
    fs::write(
        files.path().join("payments.md"),
        "PaymentHandler processes refunds and invoices",
    )
    .unwrap();
    fs::write(
        files.path().join("auth.rs"),
        "fn verify_token(jwt: &str) -> bool { true }",
    )
    .unwrap();
    (home, files)
}

#[test]
fn json_output_is_valid_json_lines() {
    let (home, files) = setup();
    let output = Command::new(bm25_bin())
        .env("HOME", home.path())
        .args(["payment", files.path().to_str().unwrap(), "--json"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    for line in stdout.lines() {
        let v: serde_json::Value =
            serde_json::from_str(line).unwrap_or_else(|_| panic!("invalid JSON line: {line}"));
        assert!(v.get("path").is_some(), "missing path field");
        assert!(v.get("score").is_some(), "missing score field");
    }
}

#[test]
fn json_output_includes_context_when_requested() {
    let (home, files) = setup();

    let output = Command::new(bm25_bin())
        .env("HOME", home.path())
        .args([
            "payment",
            files.path().to_str().unwrap(),
            "--json",
            "--context",
            "150",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let line = stdout.lines().next().expect("no output");
    let v: serde_json::Value = serde_json::from_str(line).unwrap();
    assert!(v.get("context").is_some(), "missing context field");
    let context = v["context"].as_str().unwrap();
    assert!(
        context.to_lowercase().contains("payment"),
        "context should contain query term"
    );
}

#[test]
fn json_output_filters_to_matching_files_only() {
    let (home, files) = setup();

    let output = Command::new(bm25_bin())
        .env("HOME", home.path())
        .args(["payment", files.path().to_str().unwrap(), "--json"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let values: Vec<serde_json::Value> = stdout
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();

    assert!(values
        .iter()
        .any(|v| v["path"].as_str().unwrap_or("").contains("payments.md")));
    assert!(!values
        .iter()
        .any(|v| v["path"].as_str().unwrap_or("").contains("auth.rs")));
}

#[test]
fn query_registers_absolute_path() {
    let (home, files) = setup();

    let out = Command::new(bm25_bin())
        .env("HOME", home.path())
        .args(["payment", files.path().to_str().unwrap()])
        .output()
        .unwrap();
    assert!(out.status.success());

    let sources_json =
        fs::read_to_string(home.path().join(".bm25/sources.json")).expect("sources.json missing");
    let sources: serde_json::Value = serde_json::from_str(&sources_json).unwrap();
    let uris: Vec<&str> = sources
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|s| s["uri"].as_str())
        .collect();

    let canonical = files.path().canonicalize().unwrap();
    assert!(
        uris.iter().any(|u| *u == canonical.to_str().unwrap()),
        "expected absolute path in sources, got: {uris:?}"
    );
}

#[test]
fn remove_with_relative_path_removes_absolute_registration() {
    let (home, files) = setup();
    let abs = files.path().canonicalize().unwrap();

    cmd(&["payment", abs.to_str().unwrap()], home.path());

    let out = cmd(&["remove", abs.to_str().unwrap()], home.path());
    assert!(out.status.success());

    let sources_json =
        fs::read_to_string(home.path().join(".bm25/sources.json")).expect("sources.json missing");
    let sources: serde_json::Value = serde_json::from_str(&sources_json).unwrap();
    let uris: Vec<&str> = sources
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|s| s["uri"].as_str())
        .collect();

    assert!(
        !uris.contains(&abs.to_str().unwrap()),
        "source should have been removed, got: {uris:?}"
    );
}

#[test]
fn subdir_query_does_not_create_duplicate_source() {
    let (home, files) = setup();
    let abs = files.path().canonicalize().unwrap();

    let subdir = files.path().join("sub");
    fs::create_dir(&subdir).unwrap();
    fs::write(subdir.join("note.md"), "payment note").unwrap();

    cmd(&["payment", abs.to_str().unwrap()], home.path());
    cmd(&["payment", subdir.to_str().unwrap()], home.path());

    let sources_json =
        fs::read_to_string(home.path().join(".bm25/sources.json")).expect("sources.json missing");
    let sources: serde_json::Value = serde_json::from_str(&sources_json).unwrap();
    let count = sources.as_array().unwrap().len();

    assert_eq!(count, 1, "expected 1 source, got {count}: {sources_json}");
}
