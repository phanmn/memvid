//! Tests for tier-free operation and large file handling
//!
//! This test suite verifies that:
//! 1. No tier restrictions are enforced (Free/Dev/Enterprise tiers removed)
//! 2. Large files can be processed without API key requirements
//! 3. All mutation operations work without tier checks

use memvid_core::{Memvid, PutOptions, SearchRequest};
use std::fs;
use tempfile::TempDir;

/// Test that basic operations work without any tier restrictions
#[test]
fn test_no_tier_restrictions_basic() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test_no_tier.mv2");

    // Should be able to create database without tier check
    let mut memvid = Memvid::create(&db_path).unwrap();
    
    // Should be able to put data without tier check
    let content = "This is test content for tier-free operation.";
    let opts = PutOptions {
        uri: Some("mv2://test".to_string()),
        search_text: Some(content.to_string()),
        ..Default::default()
    };
    memvid.put_bytes_with_options(content.as_bytes(), opts).unwrap();
    memvid.commit().unwrap();
    
    // Should be able to search without tier check
    let mut memvid = Memvid::open_read_only(&db_path).unwrap();
    let results = memvid.search(SearchRequest {
        query: "test content".to_string(),
        top_k: 10,
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
        acl_enforcement_mode: memvid_core::types::AclEnforcementMode::Audit,
    }).unwrap();
    
    assert!(!results.hits.is_empty(), "Should find results without tier restrictions");
    
    println!("✓ Basic operations work without tier restrictions");
}

/// Test that large files (>10MB) can be processed without API key
#[test]
fn test_large_file_no_api_key() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test_large_file.mv2");
    
    println!("Creating database...");
    let mut memvid = Memvid::create(&db_path).unwrap();
    println!("Database created");
    
    // Create a large content (1MB for faster testing)
    println!("Creating 1MB content...");
    let large_content = "x".repeat(1024 * 1024); // 1MB
    println!("Content created, putting...");
    
    // Should be able to put large content without API key requirement
    let opts = PutOptions {
        uri: Some("mv2://large".to_string()),
        search_text: Some("large file content".to_string()),
        ..Default::default()
    };
    memvid.put_bytes_with_options(large_content.as_bytes(), opts).unwrap();
    println!("Put completed, committing...");
    memvid.commit().unwrap();
    println!("Commit completed");
    
    // Verify file was created and has content
    let file_size = std::fs::metadata(&db_path).unwrap().len();
    assert!(file_size > 0, "File should have content");
    
    println!("✓ Large file (1MB) processed without API key, file size: {} bytes", file_size);
}

/// Test multiple large file operations without tier limits
#[test]
fn test_multiple_large_files_no_limits() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test_multiple_large.mv2");
    
    let mut memvid = Memvid::create(&db_path).unwrap();
    
    // Insert multiple large documents (1MB each for faster testing)
    for i in 0..5 {
        let content = format!("Document {} with large content: {}", i, "y".repeat(1024 * 1024)); // 1MB each
        let opts = PutOptions {
            uri: Some(format!("mv2://doc_{}", i)),
            search_text: Some(format!("Document {}", i)),
            ..Default::default()
        };
        memvid.put_bytes_with_options(content.as_bytes(), opts).unwrap();
    }
    memvid.commit().unwrap();
    
    // Verify file size
    let file_size = std::fs::metadata(&db_path).unwrap().len();
    println!("✓ Multiple large files (5MB total) processed without tier limits, file size: {} bytes", file_size);
}

/// Test operations with many documents without tier limits
#[test]
fn test_many_documents_no_tier() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test_many_docs.mv2");
    
    let mut memvid = Memvid::create(&db_path).unwrap();
    
    // Insert many documents
    for i in 0..100 {
        let content = format!("Common search term document number {}", i);
        let opts = PutOptions {
            uri: Some(format!("mv2://search_doc_{}", i)),
            search_text: Some(content.clone()),
            ..Default::default()
        };
        memvid.put_bytes_with_options(content.as_bytes(), opts).unwrap();
    }
    memvid.commit().unwrap();
    
    // Verify file was created
    let file_size = std::fs::metadata(&db_path).unwrap().len();
    println!("✓ Many documents (100) stored without tier restrictions, file size: {} bytes", file_size);
}

/// Test that batch operations work without tier limits
#[test]
fn test_batch_operations_no_tier() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test_batch_no_tier.mv2");
    
    let mut memvid = Memvid::create(&db_path).unwrap();
    
    // Create batch of documents
    for i in 0..50 {
        let content = format!("Batch document {}", i);
        let opts = PutOptions {
            uri: Some(format!("mv2://batch_{}", i)),
            search_text: Some(content.clone()),
            ..Default::default()
        };
        memvid.put_bytes_with_options(content.as_bytes(), opts).unwrap();
    }
    memvid.commit().unwrap();
    
    // Verify file was created
    let file_size = std::fs::metadata(&db_path).unwrap().len();
    println!("✓ Batch operations (50 documents) work without tier restrictions, file size: {} bytes", file_size);
}

/// Test file size growth without tier-based capacity limits
#[test]
fn test_file_growth_no_capacity_limits() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test_file_growth.mv2");
    
    let mut memvid = Memvid::create(&db_path).unwrap();
    
    // Continuously add data to grow the file (10 docs of 1MB each)
    for i in 0..10 {
        let content = "z".repeat(1024 * 1024); // 1MB per document
        let opts = PutOptions {
            uri: Some(format!("mv2://growth_{}", i)),
            search_text: Some("growth document".to_string()),
            ..Default::default()
        };
        memvid.put_bytes_with_options(content.as_bytes(), opts).unwrap();
    }
    memvid.commit().unwrap();
    
    // Check file size
    let file_size = fs::metadata(&db_path).unwrap().len();
    println!("✓ File grew to {} bytes without capacity limits", file_size);
}

/// Test that all mutation types work without tier checks
#[test]
fn test_all_mutation_types_no_tier() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test_all_mutations.mv2");
    
    let mut memvid = Memvid::create(&db_path).unwrap();
    
    // Test different content types
    let test_cases = vec![
        ("text", "Plain text content".as_bytes(), "text content"),
        ("json", r#"{"key": "value", "number": 42}"#.as_bytes(), "json content"),
        ("xml", "<?xml version=\"1.0\"?><root><item>Test</item></root>".as_bytes(), "xml content"),
        ("html", "<html><body><p>HTML content</p></body></html>".as_bytes(), "html content"),
        ("csv", "name,value\nitem1,100\nitem2,200".as_bytes(), "csv content"),
    ];
    
    for (key, content, search_text) in test_cases {
        let opts = PutOptions {
            uri: Some(format!("mv2://{}", key)),
            search_text: Some(search_text.to_string()),
            ..Default::default()
        };
        memvid.put_bytes_with_options(content, opts).unwrap();
    }
    memvid.commit().unwrap();
    
    // Verify file was created
    let file_size = std::fs::metadata(&db_path).unwrap().len();
    println!("✓ All mutation types work without tier restrictions, file size: {} bytes", file_size);
}

/// Integration test: complete workflow without tier restrictions
#[test]
fn test_complete_workflow_no_tier() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test_complete_workflow.mv2");
    
    // Create
    let mut memvid = Memvid::create(&db_path).unwrap();
    
    // Write various sizes
    let opts_small = PutOptions {
        uri: Some("mv2://small".to_string()),
        search_text: Some("Small content".to_string()),
        ..Default::default()
    };
    memvid.put_bytes_with_options("Small content".as_bytes(), opts_small).unwrap();
    
    let opts_medium = PutOptions {
        uri: Some("mv2://medium".to_string()),
        search_text: Some("medium file".to_string()),
        ..Default::default()
    };
    memvid.put_bytes_with_options("m".repeat(1024 * 1024).as_bytes(), opts_medium).unwrap(); // 1MB
    
    let opts_large = PutOptions {
        uri: Some("mv2://large".to_string()),
        search_text: Some("large file".to_string()),
        ..Default::default()
    };
    memvid.put_bytes_with_options("l".repeat(5 * 1024 * 1024).as_bytes(), opts_large).unwrap(); // 5MB
    
    memvid.commit().unwrap();
    
    // Close and reopen
    drop(memvid);
    
    let _memvid = Memvid::open_read_only(&db_path).unwrap();
    
    // Verify file persists
    let file_size = std::fs::metadata(&db_path).unwrap().len();
    println!("✓ Complete workflow (create, write, reopen) works without tier restrictions, file size: {} bytes", file_size);
}

/// Test very large file (10MB+) without tier restrictions
#[test]
fn test_very_large_file_no_tier() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test_very_large.mv2");
    
    let mut memvid = Memvid::create(&db_path).unwrap();
    
    // Create very large content (10MB)
    let very_large_content = "v".repeat(10 * 1024 * 1024);
    
    let opts = PutOptions {
        uri: Some("mv2://very_large".to_string()),
        search_text: Some("very large file content".to_string()),
        ..Default::default()
    };
    
    // Should succeed without any tier/API key restrictions
    memvid.put_bytes_with_options(very_large_content.as_bytes(), opts).unwrap();
    memvid.commit().unwrap();
    
    // Verify file size
    let file_size = fs::metadata(&db_path).unwrap().len();
    println!("✓ Very large file (10MB+) stored successfully, file size: {} bytes", file_size);
}
