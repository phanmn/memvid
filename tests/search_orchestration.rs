//! Integration tests for search: ingest -> index -> search -> verify results

use memvid_core::{AclEnforcementMode, Memvid, PutOptions, SearchRequest};
use tempfile::TempDir;

/// Create a populated memory file with lex indexing enabled.
fn create_and_populate(dir: &TempDir) -> std::path::PathBuf {
    let path = dir.path().join("search_test.mv2");
    let mut mem = Memvid::create(&path).unwrap();
    mem.enable_lex().unwrap();

    let opts = PutOptions::builder()
        .auto_tag(false)
        .extract_dates(false)
        .extract_triplets(false)
        .build();

    mem.put_bytes_with_options(
        b"Rust is a systems programming language focused on safety",
        opts.clone(),
    )
    .unwrap();
    mem.put_bytes_with_options(
        b"Python is great for data science and machine learning",
        opts.clone(),
    )
    .unwrap();
    mem.put_bytes_with_options(
        b"JavaScript powers the modern web with frameworks like React",
        opts,
    )
    .unwrap();
    mem.commit().unwrap();
    drop(mem);

    path
}

/// Helper to build a simple search request.
fn search_request(query: &str, top_k: usize) -> SearchRequest {
    SearchRequest {
        query: query.to_string(),
        top_k,
        snippet_chars: 200,
        uri: None,
        scope: None,
        cursor: None,
        #[cfg(feature = "temporal_track")]
        temporal: None,
        as_of_frame: None,
        as_of_ts: None,
        no_sketch: false,
        acl_context: None,
        acl_enforcement_mode: AclEnforcementMode::Audit,
    }
}

#[test]
#[cfg(feature = "lex")]
fn search_finds_relevant_document() {

    let dir = TempDir::new().unwrap();
    let path = create_and_populate(&dir);

    let mut mem = Memvid::open_read_only(&path).unwrap();
    let results = mem.search(search_request("Rust safety", 10)).unwrap();
    assert!(!results.hits.is_empty());
}

#[test]
#[cfg(feature = "lex")]
fn search_respects_limit() {

    let dir = TempDir::new().unwrap();
    let path = create_and_populate(&dir);

    let mut mem = Memvid::open_read_only(&path).unwrap();
    let results = mem.search(search_request("programming", 1)).unwrap();
    assert!(results.hits.len() <= 1);
}

#[test]
#[cfg(feature = "lex")]
fn search_no_results_for_unrelated_query() {

    let dir = TempDir::new().unwrap();
    let path = create_and_populate(&dir);

    let mut mem = Memvid::open_read_only(&path).unwrap();
    let results = mem
        .search(search_request("quantum physics supercollider", 10))
        .unwrap();
    assert!(results.hits.is_empty(), "unrelated query should return no hits");
}

#[test]
#[cfg(feature = "lex")]
fn search_after_delete_excludes_deleted() {

    let dir = TempDir::new().unwrap();
    let path = dir.path().join("delete_search.mv2");

    // Write phase: create, enable lex, put, commit, delete, commit
    let frame_id;
    {
        let mut mem = Memvid::create(&path).unwrap();
        mem.enable_lex().unwrap();

        let opts = PutOptions::builder()
            .uri("mv2://xyzzy-doc")
            .auto_tag(false)
            .extract_dates(false)
            .extract_triplets(false)
            .build();

        mem.put_bytes_with_options(b"Unique findable content xyzzy", opts)
            .unwrap();
        mem.commit().unwrap();

        let frame = mem.frame_by_uri("mv2://xyzzy-doc").unwrap();
        frame_id = frame.id;

        mem.delete_frame(frame_id).unwrap();
        mem.commit().unwrap();
    }

    // Read phase: reopen read-only and verify deleted frame is excluded
    let mut mem = Memvid::open_read_only(&path).unwrap();
    let results = mem.search(search_request("xyzzy", 10)).unwrap();
    for hit in &results.hits {
        assert_ne!(hit.frame_id, frame_id);
    }
}
