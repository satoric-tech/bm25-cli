use tempfile::TempDir;

fn search(index_dir: &std::path::Path, query: &str) -> Vec<String> {
    use tantivy::collector::TopDocs;
    use tantivy::query::QueryParser;
    use tantivy::schema::Value;

    let idx = bm25_cli::index::BM25Index::open_at(index_dir).unwrap();
    let tantivy_index = idx.open_or_create_tantivy().unwrap();

    let reader = tantivy_index.reader().unwrap();
    let searcher = reader.searcher();

    let query_parser = QueryParser::for_index(&tantivy_index, vec![idx.field_content]);
    let parsed = query_parser.parse_query(query).unwrap();
    let top_docs = searcher
        .search(&parsed, &TopDocs::with_limit(50).order_by_score())
        .unwrap();

    top_docs
        .into_iter()
        .map(|(_, addr)| {
            let doc: tantivy::TantivyDocument = searcher.doc(addr).unwrap();
            doc.get_first(idx.field_path)
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string()
        })
        .collect()
}

fn index_file(index_dir: &std::path::Path, path: &str, content: &str) {
    use tantivy::TantivyDocument;

    let idx = bm25_cli::index::BM25Index::open_at(index_dir).unwrap();
    let tantivy_index = idx.open_or_create_tantivy().unwrap();
    let mut writer = tantivy_index.writer(15_000_000).unwrap();

    let mut doc = TantivyDocument::default();
    doc.add_text(idx.field_path, path);
    doc.add_text(idx.field_content, content);
    writer.add_document(doc).unwrap();
    writer.commit().unwrap();
}

#[test]
fn camel_case_match() {
    let dir = TempDir::new().unwrap();
    index_file(
        dir.path(),
        "payments.rs",
        "fn handle_payment(amount: u64) { PaymentHandler::new() }",
    );
    let results = search(dir.path(), "payment");
    assert!(
        results.contains(&"payments.rs".to_string()),
        "should match PaymentHandler via 'payment'"
    );
}

#[test]
fn snake_case_match() {
    let dir = TempDir::new().unwrap();
    index_file(
        dir.path(),
        "auth.rs",
        "fn verify_token(jwt: &str) -> bool { todo!() }",
    );
    let results = search(dir.path(), "token");
    assert!(
        results.contains(&"auth.rs".to_string()),
        "should match verify_token via 'token'"
    );
}

#[test]
fn screaming_snake_match() {
    let dir = TempDir::new().unwrap();
    index_file(dir.path(), "config.rs", "const MAX_RETRY_COUNT: u32 = 3;");
    let results = search(dir.path(), "retry");
    assert!(
        results.contains(&"config.rs".to_string()),
        "should match MAX_RETRY_COUNT via 'retry'"
    );
}

#[test]
fn dot_separated_match() {
    let dir = TempDir::new().unwrap();
    index_file(
        dir.path(),
        "events.md",
        "payment.succeeded triggers the fulfillment flow",
    );
    let results = search(dir.path(), "succeeded");
    assert!(
        results.contains(&"events.md".to_string()),
        "should match payment.succeeded via 'succeeded'"
    );
}

#[test]
fn multi_term_ranks_best_match_first() {
    let dir = TempDir::new().unwrap();
    index_file(
        dir.path(),
        "billing.rs",
        "fn process_invoice(payment: Payment) {}",
    );
    index_file(dir.path(), "unrelated.rs", "fn foo() {}");
    let results = search(dir.path(), "invoice payment");
    assert_eq!(results[0], "billing.rs", "billing.rs should rank first");
}

#[test]
fn no_match_returns_empty() {
    let dir = TempDir::new().unwrap();
    index_file(dir.path(), "readme.md", "This project does authentication");
    let results = search(dir.path(), "kubernetes");
    assert!(results.is_empty());
}
