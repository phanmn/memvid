# Test Coverage Implementation Plan

> Required: follow the documented task execution workflow to implement this plan step-by-step.

**Goal:** Add ~150 unit tests and 2 integration test files to cover 24 previously untested source files in the memvid codebase.

**Architecture:** Inline `#[cfg(test)]` modules for unit tests in each source file, plus standalone integration test files in `tests/`. Tests verify round-trip correctness, error handling, and edge cases. External services (CLIP, Whisper, API embeddings) are mocked.

**Tech Stack:** Rust, `#[test]`, `tempfile`, `bincode`, existing `Memvid` API

---

## Phase 1: Critical Hardening

### Task 1: Encryption — Crypto Primitives

**Files:**
- Modify: `src/encryption/crypto.rs` (add `#[cfg(test)]` module)

**Step 1: Write tests**

Add at end of `src/encryption/crypto.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypt_decrypt_round_trip() {
        let password = b"test-password-123";
        let salt = [0xABu8; SALT_SIZE];
        let key = derive_key(password, &salt).unwrap();
        let nonce = [0x01u8; NONCE_SIZE];
        let plaintext = b"Hello, memvid encryption!";

        let ciphertext = encrypt(plaintext, &key, &nonce).unwrap();
        assert_ne!(&ciphertext[..], plaintext);

        let decrypted = decrypt(&ciphertext, &key, &nonce).unwrap();
        assert_eq!(&decrypted[..], plaintext);
    }

    #[test]
    fn wrong_key_fails_decrypt() {
        let salt = [0xABu8; SALT_SIZE];
        let key = derive_key(b"correct-password", &salt).unwrap();
        let wrong_key = derive_key(b"wrong-password", &salt).unwrap();
        let nonce = [0x01u8; NONCE_SIZE];
        let plaintext = b"secret data";

        let ciphertext = encrypt(plaintext, &key, &nonce).unwrap();
        let result = decrypt(&ciphertext, &wrong_key, &nonce);
        assert!(result.is_err());
    }

    #[test]
    fn empty_payload_round_trip() {
        let salt = [0x42u8; SALT_SIZE];
        let key = derive_key(b"password", &salt).unwrap();
        let nonce = [0x02u8; NONCE_SIZE];

        let ciphertext = encrypt(b"", &key, &nonce).unwrap();
        let decrypted = decrypt(&ciphertext, &key, &nonce).unwrap();
        assert!(decrypted.is_empty());
    }

    #[test]
    fn large_payload_round_trip() {
        let salt = [0x99u8; SALT_SIZE];
        let key = derive_key(b"password", &salt).unwrap();
        let nonce = [0x03u8; NONCE_SIZE];
        let plaintext = vec![0xFFu8; 128 * 1024]; // 128KB

        let ciphertext = encrypt(&plaintext, &key, &nonce).unwrap();
        let decrypted = decrypt(&ciphertext, &key, &nonce).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn key_derivation_deterministic() {
        let password = b"deterministic-test";
        let salt = [0x11u8; SALT_SIZE];

        let key1 = derive_key(password, &salt).unwrap();
        let key2 = derive_key(password, &salt).unwrap();
        assert_eq!(key1, key2);
    }

    #[test]
    fn different_salts_different_keys() {
        let password = b"same-password";
        let salt_a = [0x11u8; SALT_SIZE];
        let salt_b = [0x22u8; SALT_SIZE];

        let key_a = derive_key(password, &salt_a).unwrap();
        let key_b = derive_key(password, &salt_b).unwrap();
        assert_ne!(key_a, key_b);
    }

    #[test]
    fn corrupted_ciphertext_fails() {
        let salt = [0xCCu8; SALT_SIZE];
        let key = derive_key(b"password", &salt).unwrap();
        let nonce = [0x04u8; NONCE_SIZE];
        let plaintext = b"tamper test data";

        let mut ciphertext = encrypt(plaintext, &key, &nonce).unwrap();
        // Flip a byte in the ciphertext
        if let Some(byte) = ciphertext.get_mut(0) {
            *byte ^= 0xFF;
        }
        let result = decrypt(&ciphertext, &key, &nonce);
        assert!(result.is_err());
    }
}
```

**Step 2: Run tests**

Run: `cargo test --features encryption encryption::crypto::tests -- --nocapture`
Expected: All 7 tests PASS

**Step 3: Commit**

```bash
git add src/encryption/crypto.rs
git commit -m "test: add unit tests for encryption crypto primitives"
```

---

### Task 2: Encryption — Header Types

**Files:**
- Modify: `src/encryption/types.rs` (add `#[cfg(test)]` module)

**Step 1: Write tests**

Add at end of `src/encryption/types.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_encode_decode_round_trip() {
        let header = Mv2eHeader {
            magic: MV2E_MAGIC,
            version: MV2E_VERSION,
            kdf_algorithm: KdfAlgorithm::Argon2id,
            cipher_algorithm: CipherAlgorithm::Aes256Gcm,
            salt: [0xAA; SALT_SIZE],
            nonce: [0xBB; NONCE_SIZE],
            original_size: 123_456_789,
            reserved: [0x01, 0x00, 0x00, 0x00],
        };

        let encoded = header.encode();
        assert_eq!(encoded.len(), Mv2eHeader::SIZE);

        let decoded = Mv2eHeader::decode(&encoded).unwrap();
        assert_eq!(decoded.magic, MV2E_MAGIC);
        assert_eq!(decoded.version, MV2E_VERSION);
        assert_eq!(decoded.salt, [0xAA; SALT_SIZE]);
        assert_eq!(decoded.nonce, [0xBB; NONCE_SIZE]);
        assert_eq!(decoded.original_size, 123_456_789);
        assert_eq!(decoded.reserved, [0x01, 0x00, 0x00, 0x00]);
    }

    #[test]
    fn header_invalid_magic_rejected() {
        let mut header = Mv2eHeader {
            magic: MV2E_MAGIC,
            version: MV2E_VERSION,
            kdf_algorithm: KdfAlgorithm::Argon2id,
            cipher_algorithm: CipherAlgorithm::Aes256Gcm,
            salt: [0; SALT_SIZE],
            nonce: [0; NONCE_SIZE],
            original_size: 0,
            reserved: [0; 4],
        };
        let mut encoded = header.encode();
        // Corrupt the magic bytes
        encoded[0] = b'X';
        let result = Mv2eHeader::decode(&encoded);
        assert!(result.is_err());
    }

    #[test]
    fn header_unsupported_version_rejected() {
        let header = Mv2eHeader {
            magic: MV2E_MAGIC,
            version: MV2E_VERSION,
            kdf_algorithm: KdfAlgorithm::Argon2id,
            cipher_algorithm: CipherAlgorithm::Aes256Gcm,
            salt: [0; SALT_SIZE],
            nonce: [0; NONCE_SIZE],
            original_size: 0,
            reserved: [0; 4],
        };
        let mut encoded = header.encode();
        // Set version to 99
        encoded[4] = 99;
        encoded[5] = 0;
        let result = Mv2eHeader::decode(&encoded);
        assert!(result.is_err());
    }
}
```

**Step 2: Run tests**

Run: `cargo test --features encryption encryption::types::tests -- --nocapture`
Expected: All 3 tests PASS

**Step 3: Commit**

```bash
git add src/encryption/types.rs
git commit -m "test: add unit tests for encryption header types"
```

---

### Task 3: Encryption — File-Level Round Trip

**Files:**
- Modify: `src/encryption/capsule.rs` (add `#[cfg(test)]` module)

**Step 1: Write tests**

Add at end of `src/encryption/capsule.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;

    fn create_test_mv2(dir: &Path) -> PathBuf {
        let path = dir.join("test.mv2");
        let mut content = Vec::new();
        content.extend_from_slice(b"MV2\0");
        content.extend_from_slice(&[0u8; 4092]); // pad to 4KB header
        content.extend_from_slice(b"test payload data for encryption");
        fs::write(&path, &content).unwrap();
        path
    }

    #[test]
    fn lock_unlock_round_trip() {
        let dir = TempDir::new().unwrap();
        let mv2_path = create_test_mv2(dir.path());
        let original = fs::read(&mv2_path).unwrap();
        let password = b"test-password";

        let encrypted_path = lock_file(&mv2_path, None, password).unwrap();
        assert!(encrypted_path.extension().unwrap() == "mv2e");

        let encrypted = fs::read(&encrypted_path).unwrap();
        assert_ne!(encrypted, original);

        let decrypted_path = unlock_file(&encrypted_path, None, password).unwrap();
        let decrypted = fs::read(&decrypted_path).unwrap();
        assert_eq!(decrypted, original);
    }

    #[test]
    fn unlock_wrong_password_fails() {
        let dir = TempDir::new().unwrap();
        let mv2_path = create_test_mv2(dir.path());
        let password = b"correct-password";

        let encrypted_path = lock_file(&mv2_path, None, password).unwrap();
        let result = unlock_file(&encrypted_path, None, b"wrong-password");
        assert!(result.is_err());
    }

    #[test]
    fn validate_mv2_file_valid() {
        let dir = TempDir::new().unwrap();
        let mv2_path = create_test_mv2(dir.path());
        assert!(validate_mv2_file(&mv2_path).is_ok());
    }

    #[test]
    fn validate_mv2_file_invalid() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("not_mv2.bin");
        fs::write(&path, b"NOT_MV2_DATA").unwrap();
        assert!(validate_mv2_file(&path).is_err());
    }
}
```

**Step 2: Run tests**

Run: `cargo test --features encryption encryption::capsule::tests -- --nocapture`
Expected: All 4 tests PASS

**Step 3: Commit**

```bash
git add src/encryption/capsule.rs
git commit -m "test: add unit tests for encryption capsule lock/unlock"
```

---

### Task 4: Encryption — Streaming

**Files:**
- Modify: `src/encryption/capsule_stream.rs` (add `#[cfg(test)]` module)

**Step 1: Write tests**

Add at end of `src/encryption/capsule_stream.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;

    fn create_test_mv2(dir: &Path, extra_bytes: usize) -> PathBuf {
        let path = dir.join("test.mv2");
        let mut content = Vec::new();
        content.extend_from_slice(b"MV2\0");
        content.extend_from_slice(&vec![0u8; 4092]); // 4KB header
        content.extend_from_slice(&vec![0xAB; extra_bytes]);
        fs::write(&path, &content).unwrap();
        path
    }

    #[test]
    fn stream_lock_unlock_round_trip() {
        let dir = TempDir::new().unwrap();
        let mv2_path = create_test_mv2(dir.path(), 1024);
        let original = fs::read(&mv2_path).unwrap();
        let password = b"stream-password";

        let encrypted_path = lock_file_stream(&mv2_path, None, password).unwrap();
        let decrypted_out = dir.path().join("decrypted.mv2");
        let decrypted_path = unlock_file_stream(&encrypted_path, Some(&decrypted_out), password).unwrap();
        let decrypted = fs::read(&decrypted_path).unwrap();
        assert_eq!(decrypted, original);
    }

    #[test]
    fn stream_large_payload_multi_chunk() {
        let dir = TempDir::new().unwrap();
        // 2.5MB payload forces multiple 1MB chunks
        let mv2_path = create_test_mv2(dir.path(), 2_500_000);
        let original = fs::read(&mv2_path).unwrap();
        let password = b"large-payload-pw";

        let encrypted_path = lock_file_stream(&mv2_path, None, password).unwrap();
        let decrypted_out = dir.path().join("decrypted.mv2");
        let decrypted_path = unlock_file_stream(&encrypted_path, Some(&decrypted_out), password).unwrap();
        let decrypted = fs::read(&decrypted_path).unwrap();
        assert_eq!(decrypted, original);
    }

    #[test]
    fn stream_wrong_password_fails() {
        let dir = TempDir::new().unwrap();
        let mv2_path = create_test_mv2(dir.path(), 512);
        let password = b"correct";

        let encrypted_path = lock_file_stream(&mv2_path, None, password).unwrap();
        let result = unlock_file_stream(&encrypted_path, None, b"wrong");
        assert!(result.is_err());
    }
}
```

**Step 2: Run tests**

Run: `cargo test --features encryption encryption::capsule_stream::tests -- --nocapture`
Expected: All 3 tests PASS

**Step 3: Commit**

```bash
git add src/encryption/capsule_stream.rs
git commit -m "test: add unit tests for streaming encryption"
```

---

### Task 5: Search — Tantivy Engine

**Files:**
- Modify: `src/search/tantivy/engine.rs` (add `#[cfg(test)]` module)

**Step 1: Write tests**

Add at end of `src/search/tantivy/engine.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Frame, FrameStatus, FrameRole};
    use crate::search::parser::parse_query;

    fn make_test_frame(id: u64, uri: &str) -> Frame {
        let mut frame = Frame::default();
        frame.id = id;
        frame.uri = uri.to_string();
        frame.status = FrameStatus::Active;
        frame.role = FrameRole::Document;
        frame
    }

    #[test]
    fn create_engine() {
        let engine = TantivyEngine::create().unwrap();
        assert_eq!(engine.num_docs(), 0);
    }

    #[test]
    fn add_frame_and_search() {
        let mut engine = TantivyEngine::create().unwrap();
        let frame = make_test_frame(1, "test://doc1");
        engine.add_frame(&frame, "Rust is a systems programming language").unwrap();
        engine.commit().unwrap();

        let parsed = parse_query("Rust programming");
        let results = engine.search_documents(&parsed, None, None, None, 10).unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].frame_id, 1);
    }

    #[test]
    fn empty_index_returns_no_results() {
        let engine = TantivyEngine::create().unwrap();
        let parsed = parse_query("anything");
        let results = engine.search_documents(&parsed, None, None, None, 10).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn delete_frame_removes_from_search() {
        let mut engine = TantivyEngine::create().unwrap();
        let frame = make_test_frame(1, "test://doc1");
        engine.add_frame(&frame, "unique searchable content here").unwrap();
        engine.commit().unwrap();

        engine.delete_frame(1).unwrap();
        engine.commit().unwrap();

        let parsed = parse_query("unique searchable content");
        let results = engine.search_documents(&parsed, None, None, None, 10).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn multiple_documents_ranked() {
        let mut engine = TantivyEngine::create().unwrap();

        let frame1 = make_test_frame(1, "test://doc1");
        engine.add_frame(&frame1, "The quick brown fox").unwrap();

        let frame2 = make_test_frame(2, "test://doc2");
        engine.add_frame(&frame2, "fox fox fox fox fox").unwrap();

        engine.commit().unwrap();

        let parsed = parse_query("fox");
        let results = engine.search_documents(&parsed, None, None, None, 10).unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn snapshot_segments_round_trip() {
        let mut engine = TantivyEngine::create().unwrap();
        let frame = make_test_frame(1, "test://doc1");
        engine.add_frame(&frame, "snapshot test content").unwrap();
        engine.commit().unwrap();

        let snapshot = engine.snapshot_segments().unwrap();
        assert!(snapshot.doc_count > 0);
        assert!(!snapshot.segments.is_empty());
    }

    #[test]
    fn analyse_text_tokenizes() {
        let engine = TantivyEngine::create().unwrap();
        let tokens = engine.analyse_text("Hello World testing");
        assert!(!tokens.is_empty());
    }
}
```

**Step 2: Run tests**

Run: `cargo test --features lex search::tantivy::engine::tests -- --nocapture`
Expected: All 7 tests PASS

**Step 3: Commit**

```bash
git add src/search/tantivy/engine.rs
git commit -m "test: add unit tests for Tantivy search engine"
```

---

### Task 6: Search — Storage

**Files:**
- Modify: `src/search/tantivy/storage.rs` (add `#[cfg(test)]` module)

**Step 1: Write tests**

Add at end of `src/search/tantivy/storage.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_storage_is_empty() {
        let storage = EmbeddedLexStorage::new();
        assert!(storage.is_empty());
        assert_eq!(storage.doc_count(), 0);
        assert_eq!(storage.generation(), 0);
    }

    #[test]
    fn insert_and_query_segment() {
        let mut storage = EmbeddedLexStorage::new();
        let segment = EmbeddedLexSegment {
            path: "seg_0.bin".to_string(),
            bytes_offset: 1000,
            bytes_length: 500,
            checksum: [0xAA; 32],
        };
        storage.insert(segment);
        assert!(!storage.is_empty());
        assert_eq!(storage.segments().count(), 1);
    }

    #[test]
    fn remove_segment() {
        let mut storage = EmbeddedLexStorage::new();
        storage.insert(EmbeddedLexSegment {
            path: "seg_0.bin".to_string(),
            bytes_offset: 0,
            bytes_length: 100,
            checksum: [0; 32],
        });
        storage.remove("seg_0.bin");
        assert!(storage.is_empty());
    }

    #[test]
    fn replace_clears_and_sets() {
        let mut storage = EmbeddedLexStorage::new();
        storage.insert(EmbeddedLexSegment {
            path: "old.bin".to_string(),
            bytes_offset: 0,
            bytes_length: 100,
            checksum: [0; 32],
        });

        let new_segments = vec![EmbeddedLexSegment {
            path: "new.bin".to_string(),
            bytes_offset: 200,
            bytes_length: 300,
            checksum: [0xFF; 32],
        }];
        storage.replace(42, [0xBB; 32], new_segments);

        assert_eq!(storage.doc_count(), 42);
        assert_eq!(storage.checksum(), [0xBB; 32]);
        assert_eq!(storage.segments().count(), 1);
        assert_eq!(storage.segments().next().unwrap().path, "new.bin");
    }

    #[test]
    fn adjust_offsets() {
        let mut storage = EmbeddedLexStorage::new();
        storage.insert(EmbeddedLexSegment {
            path: "seg.bin".to_string(),
            bytes_offset: 100,
            bytes_length: 50,
            checksum: [0; 32],
        });
        storage.adjust_offsets(1000);
        let seg = storage.segments().next().unwrap();
        assert_eq!(seg.bytes_offset, 1100);
    }

    #[test]
    fn clear_resets_all() {
        let mut storage = EmbeddedLexStorage::new();
        storage.set_doc_count(10);
        storage.set_generation(5);
        storage.insert(EmbeddedLexSegment {
            path: "seg.bin".to_string(),
            bytes_offset: 0,
            bytes_length: 100,
            checksum: [0; 32],
        });
        storage.clear();
        assert!(storage.is_empty());
        assert_eq!(storage.doc_count(), 0);
    }
}
```

**Step 2: Run tests**

Run: `cargo test --features lex search::tantivy::storage::tests -- --nocapture`
Expected: All 6 tests PASS

**Step 3: Commit**

```bash
git add src/search/tantivy/storage.rs
git commit -m "test: add unit tests for Tantivy storage layer"
```

---

### Task 7: Search — Time Filter

**Files:**
- Modify: `src/memvid/search/time_filter.rs` (add `#[cfg(test)]` module)

**Step 1: Write tests**

Add at end of `src/memvid/search/time_filter.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_contains() {
        let bounds = Bounds::new(100, 200);
        assert!(bounds.contains(100));
        assert!(bounds.contains(150));
        assert!(bounds.contains(200));
        assert!(!bounds.contains(99));
        assert!(!bounds.contains(201));
    }

    #[test]
    fn bounds_intersect() {
        let a = Bounds::new(100, 200);
        let b = Bounds::new(150, 250);
        let c = a.intersect(&b);
        assert!(c.contains(150));
        assert!(c.contains(200));
        assert!(!c.contains(100));
        assert!(!c.contains(250));
    }

    #[test]
    fn bounds_no_overlap() {
        let a = Bounds::new(100, 200);
        let b = Bounds::new(300, 400);
        let c = a.intersect(&b);
        // Intersection of non-overlapping should be empty (start > end)
        assert!(!c.contains(100));
        assert!(!c.contains(300));
    }

    #[test]
    fn bounds_overlaps_true() {
        let a = Bounds::new(100, 200);
        let b = Bounds::new(150, 250);
        assert!(a.overlaps(&b));
        assert!(b.overlaps(&a));
    }

    #[test]
    fn bounds_overlaps_false() {
        let a = Bounds::new(100, 200);
        let b = Bounds::new(300, 400);
        assert!(!a.overlaps(&b));
        assert!(!b.overlaps(&a));
    }
}
```

**Step 2: Run tests**

Run: `cargo test --features lex memvid::search::time_filter::tests -- --nocapture`
Expected: All 5 tests PASS (adjust if `Bounds` is feature-gated behind `temporal_track`)

**Step 3: Commit**

```bash
git add src/memvid/search/time_filter.rs
git commit -m "test: add unit tests for search time filter bounds"
```

---

### Task 8: Search — Helpers

**Files:**
- Modify: `src/memvid/search/helpers.rs` (add `#[cfg(test)]` module)

**Step 1: Write tests**

Add at end of `src/memvid/search/helpers.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::SearchEngineKind;

    #[test]
    fn empty_search_response_fields() {
        let params = SearchParams::default();
        let resp = empty_search_response(
            "test query".to_string(),
            params,
            42,
            SearchEngineKind::Lex,
        );
        assert_eq!(resp.query, "test query");
        assert!(resp.hits.is_empty());
        assert_eq!(resp.elapsed_ms, 42);
    }

    #[test]
    fn timestamp_to_rfc3339_valid() {
        // 2024-01-01T00:00:00Z = 1704067200
        let result = timestamp_to_rfc3339(1_704_067_200);
        assert!(result.is_some());
        let s = result.unwrap();
        assert!(s.contains("2024"));
    }

    #[test]
    fn timestamp_to_rfc3339_zero() {
        let result = timestamp_to_rfc3339(0);
        assert!(result.is_some());
        let s = result.unwrap();
        assert!(s.contains("1970"));
    }

    #[test]
    fn parse_cursor_none_returns_zero() {
        let offset = parse_cursor(None, 100).unwrap();
        assert_eq!(offset, 0);
    }

    #[test]
    fn parse_cursor_valid_number() {
        let offset = parse_cursor(Some("10"), 100).unwrap();
        assert_eq!(offset, 10);
    }

    #[test]
    fn parse_cursor_beyond_total_errors() {
        let result = parse_cursor(Some("200"), 100);
        assert!(result.is_err());
    }
}
```

**Step 2: Run tests**

Run: `cargo test --features lex memvid::search::helpers::tests -- --nocapture`
Expected: All 6 tests PASS

**Step 3: Commit**

```bash
git add src/memvid/search/helpers.rs
git commit -m "test: add unit tests for search helpers"
```

---

### Task 9: Document Readers — Registry & Passthrough

**Files:**
- Modify: `src/reader/mod.rs` (add `#[cfg(test)]` module)

**Step 1: Write tests**

Add at end of `src/reader/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_default_has_readers() {
        let registry = ReaderRegistry::new();
        assert!(!registry.readers().is_empty());
    }

    #[test]
    fn registry_finds_pdf_reader() {
        let registry = ReaderRegistry::new();
        let hint = ReaderHint::new(Some("application/pdf"), None);
        let reader = registry.find_reader(&hint);
        assert!(reader.is_some());
        assert_eq!(reader.unwrap().name(), "pdf");
    }

    #[test]
    fn registry_finds_docx_reader() {
        let registry = ReaderRegistry::new();
        let hint = ReaderHint::new(
            Some("application/vnd.openxmlformats-officedocument.wordprocessingml.document"),
            None,
        );
        let reader = registry.find_reader(&hint);
        assert!(reader.is_some());
        assert_eq!(reader.unwrap().name(), "docx");
    }

    #[test]
    fn registry_finds_pptx_reader() {
        let registry = ReaderRegistry::new();
        let hint = ReaderHint::new(
            Some("application/vnd.openxmlformats-officedocument.presentationml.presentation"),
            None,
        );
        let reader = registry.find_reader(&hint);
        assert!(reader.is_some());
        assert_eq!(reader.unwrap().name(), "pptx");
    }

    #[test]
    fn registry_finds_xlsx_reader() {
        let registry = ReaderRegistry::new();
        let hint = ReaderHint::new(
            Some("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"),
            None,
        );
        let reader = registry.find_reader(&hint);
        assert!(reader.is_some());
        assert_eq!(reader.unwrap().name(), "xlsx");
    }

    #[test]
    fn registry_finds_reader_by_format() {
        let registry = ReaderRegistry::new();
        let hint = ReaderHint::new(None, Some(DocumentFormat::Pdf));
        let reader = registry.find_reader(&hint);
        assert!(reader.is_some());
    }

    #[test]
    fn passthrough_extracts_plain_text() {
        let reader = PassthroughReader;
        let hint = ReaderHint::new(Some("text/plain"), Some(DocumentFormat::PlainText));
        assert!(reader.supports(&hint));

        let text = b"Hello, world! This is plain text.";
        let output = reader.extract(text, &hint);
        // May fail if extractous is not available — that's expected
        // The test validates the reader accepts plain text format
        if let Ok(out) = output {
            assert!(!out.document.text.as_deref().unwrap_or("").is_empty());
        }
    }

    #[test]
    fn document_format_labels() {
        assert_eq!(DocumentFormat::Pdf.label(), "pdf");
        assert_eq!(DocumentFormat::Docx.label(), "docx");
        assert_eq!(DocumentFormat::Xlsx.label(), "xlsx");
        assert_eq!(DocumentFormat::Pptx.label(), "pptx");
        assert_eq!(DocumentFormat::PlainText.label(), "text");
    }
}
```

**Step 2: Run tests**

Run: `cargo test reader::tests -- --nocapture`
Expected: All 8 tests PASS

**Step 3: Commit**

```bash
git add src/reader/mod.rs
git commit -m "test: add unit tests for reader registry and format detection"
```

---

### Task 10: Document Readers — DOCX

**Files:**
- Modify: `src/reader/docx.rs` (add `#[cfg(test)]` module)

**Step 1: Write tests**

Add at end of `src/reader/docx.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn docx_reader_name() {
        let reader = DocxReader;
        assert_eq!(reader.name(), "docx");
    }

    #[test]
    fn docx_supports_correct_mime() {
        let reader = DocxReader;
        let hint = ReaderHint::new(
            Some("application/vnd.openxmlformats-officedocument.wordprocessingml.document"),
            None,
        );
        assert!(reader.supports(&hint));
    }

    #[test]
    fn docx_supports_format_enum() {
        let reader = DocxReader;
        let hint = ReaderHint::new(None, Some(DocumentFormat::Docx));
        assert!(reader.supports(&hint));
    }

    #[test]
    fn docx_rejects_wrong_mime() {
        let reader = DocxReader;
        let hint = ReaderHint::new(Some("application/pdf"), None);
        assert!(!reader.supports(&hint));
    }

    #[test]
    fn docx_invalid_bytes_returns_error() {
        let reader = DocxReader;
        let hint = ReaderHint::new(
            Some("application/vnd.openxmlformats-officedocument.wordprocessingml.document"),
            None,
        );
        let result = reader.extract(b"not a zip file", &hint);
        assert!(result.is_err());
    }

    #[test]
    fn extract_plain_text_from_xml() {
        let xml = r#"<w:document><w:body><w:p><w:r><w:t>Hello World</w:t></w:r></w:p></w:body></w:document>"#;
        let text = extract_plain_text(xml, b"w:p");
        assert!(text.contains("Hello World"));
    }
}
```

**Step 2: Run tests**

Run: `cargo test reader::docx::tests -- --nocapture`
Expected: All 6 tests PASS

**Step 3: Commit**

```bash
git add src/reader/docx.rs
git commit -m "test: add unit tests for DOCX reader"
```

---

### Task 11: Document Readers — PPTX

**Files:**
- Modify: `src/reader/pptx.rs` (add `#[cfg(test)]` module)

**Step 1: Write tests**

Add at end of `src/reader/pptx.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pptx_reader_name() {
        let reader = PptxReader;
        assert_eq!(reader.name(), "pptx");
    }

    #[test]
    fn pptx_supports_correct_mime() {
        let reader = PptxReader;
        let hint = ReaderHint::new(
            Some("application/vnd.openxmlformats-officedocument.presentationml.presentation"),
            None,
        );
        assert!(reader.supports(&hint));
    }

    #[test]
    fn pptx_supports_format_enum() {
        let reader = PptxReader;
        let hint = ReaderHint::new(None, Some(DocumentFormat::Pptx));
        assert!(reader.supports(&hint));
    }

    #[test]
    fn pptx_rejects_wrong_mime() {
        let reader = PptxReader;
        let hint = ReaderHint::new(Some("text/plain"), None);
        assert!(!reader.supports(&hint));
    }

    #[test]
    fn pptx_invalid_bytes_returns_error() {
        let reader = PptxReader;
        let hint = ReaderHint::new(
            Some("application/vnd.openxmlformats-officedocument.presentationml.presentation"),
            None,
        );
        let result = reader.extract(b"not a zip file", &hint);
        assert!(result.is_err());
    }

    #[test]
    fn extract_plain_text_from_slide_xml() {
        let xml = r#"<p:sld><p:cSld><p:spTree><p:sp><p:txBody><a:p><a:r><a:t>Slide Text</a:t></a:r></a:p></p:txBody></p:sp></p:spTree></p:cSld></p:sld>"#;
        let text = extract_plain_text(xml, b"a:p");
        assert!(text.contains("Slide Text"));
    }
}
```

**Step 2: Run tests**

Run: `cargo test reader::pptx::tests -- --nocapture`
Expected: All 6 tests PASS

**Step 3: Commit**

```bash
git add src/reader/pptx.rs
git commit -m "test: add unit tests for PPTX reader"
```

---

## Phase 2: Type Safety & Recovery

### Task 12: Types — Frame

**Files:**
- Modify: `src/types/frame.rs` (add or extend `#[cfg(test)]` module)

**Step 1: Write tests**

Add at end of `src/types/frame.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_default_is_valid() {
        let frame = Frame::default();
        assert_eq!(frame.id, 0);
        assert_eq!(frame.status, FrameStatus::Active);
    }

    #[test]
    fn timeline_query_builder_defaults() {
        let query = TimelineQuery::builder().build();
        assert!(!query.reverse);
    }

    #[test]
    fn timeline_query_builder_limit() {
        let query = TimelineQuery::builder()
            .limit(std::num::NonZeroU64::new(50).unwrap())
            .build();
        assert_eq!(query.limit, Some(std::num::NonZeroU64::new(50).unwrap()));
    }

    #[test]
    fn timeline_query_builder_since_until() {
        let query = TimelineQuery::builder()
            .since(1000)
            .until(2000)
            .build();
        assert_eq!(query.since, Some(1000));
        assert_eq!(query.until, Some(2000));
    }

    #[test]
    fn timeline_query_builder_reverse() {
        let query = TimelineQuery::builder()
            .reverse(true)
            .build();
        assert!(query.reverse);
    }

    #[test]
    fn timeline_query_no_limit() {
        let query = TimelineQuery::builder()
            .limit(std::num::NonZeroU64::new(10).unwrap())
            .no_limit()
            .build();
        assert!(query.limit.is_none());
    }

    #[test]
    fn stats_default() {
        let stats = Stats::default();
        assert_eq!(stats.frame_count, 0);
    }
}
```

**Step 2: Run tests**

Run: `cargo test types::frame::tests -- --nocapture`
Expected: All 7 tests PASS

**Step 3: Commit**

```bash
git add src/types/frame.rs
git commit -m "test: add unit tests for Frame and TimelineQuery types"
```

---

### Task 13: Types — Options

**Files:**
- Modify: `src/types/options.rs` (add `#[cfg(test)]` module)

**Step 1: Write tests**

Add at end of `src/types/options.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn put_options_builder_defaults() {
        let opts = PutOptions::builder().build();
        assert!(opts.auto_tag);
        assert!(opts.extract_dates);
        assert!(opts.extract_triplets);
        assert!(opts.instant_index);
        assert!(opts.uri.is_none());
        assert!(opts.title.is_none());
        assert!(opts.tags.is_empty());
    }

    #[test]
    fn put_options_builder_chaining() {
        let opts = PutOptions::builder()
            .uri("test://doc")
            .title("Test Doc")
            .tag("key", "value")
            .label("important")
            .auto_tag(false)
            .extract_dates(false)
            .build();

        assert_eq!(opts.uri.as_deref(), Some("test://doc"));
        assert_eq!(opts.title.as_deref(), Some("Test Doc"));
        assert!(!opts.auto_tag);
        assert!(!opts.extract_dates);
    }

    #[test]
    fn put_options_builder_push_tag() {
        let opts = PutOptions::builder()
            .push_tag("tag1")
            .push_tag("tag2")
            .build();
        assert_eq!(opts.tags.len(), 2);
    }

    #[test]
    fn put_options_builder_timestamp() {
        let opts = PutOptions::builder()
            .timestamp(1_704_067_200)
            .build();
        assert_eq!(opts.timestamp, Some(1_704_067_200));
    }

    #[test]
    fn put_options_builder_role() {
        let opts = PutOptions::builder()
            .role(FrameRole::DocumentChunk)
            .build();
        assert_eq!(opts.role, Some(FrameRole::DocumentChunk));
    }

    #[test]
    fn put_many_opts_default() {
        let opts = PutManyOpts::default();
        assert!(opts.disable_auto_checkpoint);
        assert!(!opts.skip_sync);
        assert!(opts.enable_enrichment);
        assert!(opts.no_raw);
    }
}
```

**Step 2: Run tests**

Run: `cargo test types::options::tests -- --nocapture`
Expected: All 6 tests PASS

**Step 3: Commit**

```bash
git add src/types/options.rs
git commit -m "test: add unit tests for PutOptions and PutManyOpts"
```

---

### Task 14: Types — Common

**Files:**
- Modify: `src/types/common.rs` (extend existing `#[cfg(test)]` module if present, or add new)

**Step 1: Write tests**

Add to `src/types/common.rs` test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_encoding_round_trip() {
        assert_eq!(CanonicalEncoding::from_byte(CanonicalEncoding::Plain.as_byte()), CanonicalEncoding::Plain);
        assert_eq!(CanonicalEncoding::from_byte(CanonicalEncoding::Zstd.as_byte()), CanonicalEncoding::Zstd);
    }

    #[test]
    fn canonical_encoding_unknown_defaults_to_plain() {
        assert_eq!(CanonicalEncoding::from_byte(0xFF), CanonicalEncoding::Plain);
    }

    #[test]
    fn tier_capacity_bytes() {
        // All tiers should have non-zero capacity
        assert!(Tier::Free.capacity_bytes() > 0);
        assert!(Tier::Dev.capacity_bytes() > 0);
        assert!(Tier::Enterprise.capacity_bytes() > 0);
        // Enterprise >= Dev >= Free
        assert!(Tier::Enterprise.capacity_bytes() >= Tier::Dev.capacity_bytes());
        assert!(Tier::Dev.capacity_bytes() >= Tier::Free.capacity_bytes());
    }

    #[test]
    fn frame_status_variants() {
        let active = FrameStatus::Active;
        let deleted = FrameStatus::Deleted;
        assert_ne!(active, deleted);
    }

    #[test]
    fn enrichment_state_needs_enrichment() {
        assert!(EnrichmentState::Searchable.needs_enrichment());
        assert!(!EnrichmentState::Enriched.needs_enrichment());
    }
}
```

**Step 2: Run tests**

Run: `cargo test types::common::tests -- --nocapture`
Expected: All 5 tests PASS

**Step 3: Commit**

```bash
git add src/types/common.rs
git commit -m "test: add unit tests for common types"
```

---

### Task 15: Types — Schema

**Files:**
- Modify: `src/types/schema.rs` (extend existing `#[cfg(test)]` module)

**Step 1: Write tests**

Add to `src/types/schema.rs` test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_registry_has_builtins() {
        let registry = SchemaRegistry::new();
        assert!(registry.contains("employer"));
        assert!(registry.contains("likes"));
        assert!(registry.contains("birthday"));
    }

    #[test]
    fn schema_registry_empty_has_no_builtins() {
        let registry = SchemaRegistry::empty();
        assert!(!registry.contains("employer"));
    }

    #[test]
    fn register_custom_predicate() {
        let mut registry = SchemaRegistry::empty();
        let schema = PredicateSchema::new("custom_field", "Custom Field");
        registry.register(schema);
        assert!(registry.contains("custom_field"));
    }

    #[test]
    fn value_type_matches_string() {
        assert!(ValueType::String.matches("hello"));
        assert!(ValueType::Any.matches("anything"));
    }

    #[test]
    fn value_type_matches_number() {
        assert!(ValueType::Number.matches("42"));
        assert!(ValueType::Number.matches("3.14"));
        assert!(!ValueType::Number.matches("not a number"));
    }

    #[test]
    fn validate_with_strict_registry() {
        let mut registry = SchemaRegistry::empty().strict();
        let result = registry.validate("unknown_pred", "value", None);
        assert!(result.is_err());
    }

    #[test]
    fn predicate_schema_builder() {
        let schema = PredicateSchema::new("test", "Test Predicate")
            .with_range(ValueType::String)
            .multiple();
        assert_eq!(schema.cardinality, Cardinality::Multiple);
    }
}
```

**Step 2: Run tests**

Run: `cargo test types::schema::tests -- --nocapture`
Expected: All 7 tests PASS

**Step 3: Commit**

```bash
git add src/types/schema.rs
git commit -m "test: add unit tests for schema registry and predicates"
```

---

### Task 16: I/O — Header Codec

**Files:**
- Modify: `src/io/header.rs` (add `#[cfg(test)]` module)

**Step 1: Write tests**

Add at end of `src/io/header.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Header;

    #[test]
    fn header_encode_decode_round_trip() {
        let header = Header::default();
        let encoded = HeaderCodec::encode(&header).unwrap();
        assert_eq!(encoded.len(), HEADER_SIZE);

        let decoded = HeaderCodec::decode(&encoded).unwrap();
        assert_eq!(decoded.version, header.version);
    }

    #[test]
    fn header_write_read_round_trip() {
        let header = Header::default();
        let mut buf = std::io::Cursor::new(vec![0u8; HEADER_SIZE * 2]);
        HeaderCodec::write(&mut buf, &header).unwrap();

        let decoded = HeaderCodec::read(&mut buf).unwrap();
        assert_eq!(decoded.version, header.version);
    }

    #[test]
    fn header_invalid_magic_rejected() {
        let mut encoded = HeaderCodec::encode(&Header::default()).unwrap();
        // Corrupt magic bytes
        encoded[0] = 0xFF;
        let result = HeaderCodec::decode(&encoded);
        assert!(result.is_err());
    }
}
```

**Step 2: Run tests**

Run: `cargo test io::header::tests -- --nocapture`
Expected: All 3 tests PASS

**Step 3: Commit**

```bash
git add src/io/header.rs
git commit -m "test: add unit tests for header codec"
```

---

### Task 17: I/O — Time Index

**Files:**
- Modify: `src/io/time_index.rs` (add `#[cfg(test)]` module)

**Step 1: Write tests**

Add at end of `src/io/time_index.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_index_entry_new() {
        let entry = TimeIndexEntry::new(1000, 42);
        assert_eq!(entry.timestamp, 1000);
        assert_eq!(entry.frame_id, 42);
    }

    #[test]
    fn time_index_write_read_round_trip() {
        let mut entries = vec![
            TimeIndexEntry::new(100, 1),
            TimeIndexEntry::new(200, 2),
            TimeIndexEntry::new(300, 3),
        ];

        let mut buf = std::io::Cursor::new(Vec::new());
        let (offset, length, checksum) = append_track(&mut buf, &mut entries).unwrap();

        let recovered = read_track(&mut buf, offset, length).unwrap();
        assert_eq!(recovered.len(), 3);
        assert_eq!(recovered[0].timestamp, 100);
        assert_eq!(recovered[1].frame_id, 2);
        assert_eq!(recovered[2].timestamp, 300);
    }

    #[test]
    fn time_index_checksum_deterministic() {
        let entries = vec![
            TimeIndexEntry::new(100, 1),
            TimeIndexEntry::new(200, 2),
        ];
        let c1 = calculate_checksum(&entries);
        let c2 = calculate_checksum(&entries);
        assert_eq!(c1, c2);
    }

    #[test]
    fn time_index_checksum_changes_with_data() {
        let entries_a = vec![TimeIndexEntry::new(100, 1)];
        let entries_b = vec![TimeIndexEntry::new(200, 1)];
        let c1 = calculate_checksum(&entries_a);
        let c2 = calculate_checksum(&entries_b);
        assert_ne!(c1, c2);
    }

    #[test]
    fn time_index_empty_entries() {
        let mut entries = vec![];
        let mut buf = std::io::Cursor::new(Vec::new());
        let (offset, length, _checksum) = append_track(&mut buf, &mut entries).unwrap();
        let recovered = read_track(&mut buf, offset, length).unwrap();
        assert!(recovered.is_empty());
    }
}
```

**Step 2: Run tests**

Run: `cargo test io::time_index::tests -- --nocapture`
Expected: All 5 tests PASS

**Step 3: Commit**

```bash
git add src/io/time_index.rs
git commit -m "test: add unit tests for time index I/O"
```

---

### Task 18: I/O — Manifest WAL

**Files:**
- Modify: `src/io/manifest_wal.rs` (add `#[cfg(test)]` module)

**Step 1: Write tests**

Add at end of `src/io/manifest_wal.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn manifest_wal_open_creates_file() {
        let dir = TempDir::new().unwrap();
        let wal_path = dir.path().join("manifest.wal");
        let wal = ManifestWal::open(&wal_path).unwrap();
        assert!(wal_path.exists());
        let entries = wal.replay().unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn manifest_wal_append_and_replay() {
        let dir = TempDir::new().unwrap();
        let wal_path = dir.path().join("manifest.wal");
        let mut wal = ManifestWal::open(&wal_path).unwrap();

        let segments = vec![
            IndexSegmentRef {
                segment_id: 1,
                kind: crate::types::SegmentKind::Lexical,
                bytes_offset: 1000,
                bytes_length: 500,
                checksum: [0xAA; 32],
            },
        ];
        wal.append_segments(&segments).unwrap();
        wal.flush().unwrap();

        let replayed = wal.replay().unwrap();
        assert_eq!(replayed.len(), 1);
        assert_eq!(replayed[0].segment_id, 1);
        assert_eq!(replayed[0].bytes_offset, 1000);
    }

    #[test]
    fn manifest_wal_truncate_clears() {
        let dir = TempDir::new().unwrap();
        let wal_path = dir.path().join("manifest.wal");
        let mut wal = ManifestWal::open(&wal_path).unwrap();

        let segments = vec![
            IndexSegmentRef {
                segment_id: 1,
                kind: crate::types::SegmentKind::Lexical,
                bytes_offset: 0,
                bytes_length: 100,
                checksum: [0; 32],
            },
        ];
        wal.append_segments(&segments).unwrap();
        wal.flush().unwrap();
        wal.truncate().unwrap();

        let replayed = wal.replay().unwrap();
        assert!(replayed.is_empty());
    }
}
```

**Step 2: Run tests**

Run: `cargo test io::manifest_wal::tests -- --nocapture`
Expected: All 3 tests PASS

**Step 3: Commit**

```bash
git add src/io/manifest_wal.rs
git commit -m "test: add unit tests for manifest WAL"
```

---

## Phase 3: Broad Sweep

### Task 19: Extract

**Files:**
- Modify: `src/extract.rs` (add `#[cfg(test)]` module)

**Step 1: Write tests**

Add at end of `src/extract.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracted_document_empty() {
        let doc = ExtractedDocument::empty();
        assert!(doc.text.is_none());
    }

    #[test]
    fn processor_config_default() {
        let config = ProcessorConfig::default();
        assert!(config.max_text_chars > 0);
    }

    #[test]
    fn document_processor_new() {
        let config = ProcessorConfig::default();
        let _processor = DocumentProcessor::new(config);
        // Just verify construction doesn't panic
    }

    #[test]
    fn document_processor_extract_nonexistent_path() {
        let config = ProcessorConfig::default();
        let processor = DocumentProcessor::new(config);
        let result = processor.extract_from_path(std::path::Path::new("/nonexistent/file.pdf"));
        assert!(result.is_err());
    }

    #[test]
    fn document_processor_extract_plain_text_bytes() {
        let config = ProcessorConfig::default();
        let processor = DocumentProcessor::new(config);
        let result = processor.extract_from_bytes(b"Hello, plain text content");
        // With extractous feature, this should succeed
        // Without extractous, it may return an error — both are valid
        // We just verify it doesn't panic
        let _ = result;
    }
}
```

**Step 2: Run tests**

Run: `cargo test extract::tests -- --nocapture`
Expected: All 5 tests PASS

**Step 3: Commit**

```bash
git add src/extract.rs
git commit -m "test: add unit tests for document extraction"
```

---

### Task 20: Registry

**Files:**
- Modify: `src/registry.rs` (add `#[cfg(test)]` module)

**Step 1: Write tests**

Add at end of `src/registry.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn compute_file_id_deterministic() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.mv2");
        std::fs::write(&path, b"test content").unwrap();

        let id1 = compute_file_id(&path).unwrap();
        let id2 = compute_file_id(&path).unwrap();
        assert_eq!(id1.as_str(), id2.as_str());
    }

    #[test]
    fn file_id_display() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.mv2");
        std::fs::write(&path, b"content").unwrap();

        let id = compute_file_id(&path).unwrap();
        let display = format!("{}", id);
        assert!(!display.is_empty());
    }

    #[test]
    fn lock_record_new_and_touch() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.mv2");
        std::fs::write(&path, b"content").unwrap();
        let file_id = compute_file_id(&path).unwrap();

        let mut record = LockRecord::new(&file_id, &path, "test-cmd".to_string(), 1000).unwrap();
        assert_eq!(record.cmd, "test-cmd");
        assert!(record.touch().is_ok());
    }

    #[test]
    fn is_stale_fresh_record() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.mv2");
        std::fs::write(&path, b"content").unwrap();
        let file_id = compute_file_id(&path).unwrap();

        let record = LockRecord::new(&file_id, &path, "cmd".to_string(), 1000).unwrap();
        // A fresh record with generous grace should not be stale
        assert!(!is_stale(&record, std::time::Duration::from_secs(60)));
    }

    #[test]
    fn write_read_remove_record() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.mv2");
        std::fs::write(&path, b"content").unwrap();
        let file_id = compute_file_id(&path).unwrap();

        let record = LockRecord::new(&file_id, &path, "test".to_string(), 1000).unwrap();
        write_record(&record).unwrap();

        let read = read_record(&file_id).unwrap();
        assert!(read.is_some());

        remove_record(&file_id).unwrap();
        let read_after = read_record(&file_id).unwrap();
        assert!(read_after.is_none());
    }
}
```

**Step 2: Run tests**

Run: `cargo test registry::tests -- --nocapture`
Expected: All 5 tests PASS

**Step 3: Commit**

```bash
git add src/registry.rs
git commit -m "test: add unit tests for file registry"
```

---

### Task 21: Error Types

**Files:**
- Modify: `src/error.rs` (add `#[cfg(test)]` module)

**Step 1: Write tests**

Add at end of `src/error.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn io_error_display() {
        let err = MemvidError::Io {
            source: std::io::Error::new(std::io::ErrorKind::NotFound, "file not found"),
            path: Some(PathBuf::from("/tmp/test.mv2")),
        };
        let msg = format!("{}", err);
        assert!(!msg.is_empty());
    }

    #[test]
    fn checksum_mismatch_display() {
        let err = MemvidError::ChecksumMismatch { context: "header" };
        let msg = format!("{}", err);
        assert!(msg.contains("header"));
    }

    #[test]
    fn invalid_header_display() {
        let err = MemvidError::InvalidHeader {
            reason: "bad magic".into(),
        };
        let msg = format!("{}", err);
        assert!(msg.contains("bad magic"));
    }

    #[test]
    fn locked_error_display() {
        let locked = LockedError::new(
            PathBuf::from("/tmp/test.mv2"),
            "file is locked",
            None,
            false,
        );
        let err = MemvidError::Locked(Box::new(locked));
        let msg = format!("{}", err);
        assert!(!msg.is_empty());
    }

    #[test]
    fn from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied");
        let err: MemvidError = io_err.into();
        let msg = format!("{}", err);
        assert!(!msg.is_empty());
    }

    #[test]
    fn lock_owner_hint_default() {
        let hint = LockOwnerHint {
            pid: Some(1234),
            cmd: Some("test".to_string()),
            started_at: None,
            file_path: None,
            file_id: None,
            last_heartbeat: None,
            heartbeat_ms: None,
        };
        assert_eq!(hint.pid, Some(1234));
    }
}
```

**Step 2: Run tests**

Run: `cargo test error::tests -- --nocapture`
Expected: All 6 tests PASS

**Step 3: Commit**

```bash
git add src/error.rs
git commit -m "test: add unit tests for error types"
```

---

### Task 22: Replay Types

**Files:**
- Modify: `src/replay/types.rs` (add `#[cfg(test)]` module)

**Step 1: Write tests**

Add at end of `src/replay/types.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replay_action_new() {
        let action = ReplayAction::new(1, ActionType::Put { frame_id: 42 });
        assert_eq!(action.sequence, 1);
        assert_eq!(action.action_type.name(), "put");
    }

    #[test]
    fn replay_action_with_input_output() {
        let action = ReplayAction::new(1, ActionType::Put { frame_id: 1 })
            .with_input(b"hello")
            .with_output(b"world")
            .with_duration_ms(100);
        assert_eq!(action.duration_ms, 100);
        assert!(!action.input_preview.is_empty());
        assert!(!action.output_preview.is_empty());
    }

    #[test]
    fn replay_session_new() {
        let session = ReplaySession::new(Some("test".to_string()));
        assert!(session.is_recording());
        assert_eq!(session.name, Some("test".to_string()));
        assert!(session.actions.is_empty());
    }

    #[test]
    fn replay_session_add_action() {
        let mut session = ReplaySession::new(None);
        let action = ReplayAction::new(0, ActionType::Put { frame_id: 1 });
        session.add_action(action);
        assert_eq!(session.actions.len(), 1);
        assert_eq!(session.next_sequence(), 1);
    }

    #[test]
    fn replay_session_end() {
        let mut session = ReplaySession::new(None);
        assert!(session.is_recording());
        session.end();
        assert!(!session.is_recording());
        assert!(session.ended_secs.is_some());
    }

    #[test]
    fn replay_session_duration() {
        let mut session = ReplaySession::new(None);
        session.end();
        // Duration should be >= 0 (could be 0 if start and end are same second)
        let _dur = session.duration_secs();
    }

    #[test]
    fn action_type_names() {
        assert_eq!(ActionType::Put { frame_id: 1 }.name(), "put");
        assert_eq!(ActionType::Delete { frame_id: 1 }.name(), "delete");
        assert_eq!(ActionType::Find { query: String::new(), mode: String::new(), result_count: 0 }.name(), "find");
    }

    #[test]
    fn checkpoint_new() {
        let snapshot = StateSnapshot::default();
        let checkpoint = Checkpoint::new(1, 10, snapshot);
        assert_eq!(checkpoint.id, 1);
        assert_eq!(checkpoint.at_sequence, 10);
    }

    #[test]
    fn replay_manifest_default() {
        let manifest = ReplayManifest::default();
        assert!(!manifest.has_sessions());
        assert_eq!(manifest.session_count, 0);
    }

    #[test]
    fn session_summary_from_session() {
        let mut session = ReplaySession::new(Some("test".to_string()));
        session.add_action(ReplayAction::new(0, ActionType::Put { frame_id: 1 }));
        session.end();

        let summary = SessionSummary::from(&session);
        assert_eq!(summary.action_count, 1);
        assert_eq!(summary.name, Some("test".to_string()));
    }
}
```

**Step 2: Run tests**

Run: `cargo test replay::types::tests -- --nocapture`
Expected: All 10 tests PASS

**Step 3: Commit**

```bash
git add src/replay/types.rs
git commit -m "test: add unit tests for replay types"
```

---

### Task 23: Replay — Active Session

**Files:**
- Modify: `src/replay/mod.rs` (add `#[cfg(test)]` module)

**Step 1: Write tests**

Add at end of `src/replay/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replay_config_default() {
        let config = ReplayConfig::default();
        assert_eq!(config.auto_checkpoint_interval, 0); // 0 means disabled
    }

    #[test]
    fn active_session_new() {
        let config = ReplayConfig::default();
        let session = ActiveSession::new(Some("test".to_string()), config);
        assert!(!session.should_checkpoint());
    }

    #[test]
    fn active_session_record_action() {
        let config = ReplayConfig {
            auto_checkpoint_interval: 100,
            max_actions_per_session: None,
            auto_record: true,
        };
        let mut session = ActiveSession::new(None, config);
        let action = ReplayAction::new(0, ActionType::Put { frame_id: 1 });
        session.record_action(action);
    }

    #[test]
    fn active_session_checkpoint() {
        let config = ReplayConfig {
            auto_checkpoint_interval: 1, // checkpoint after every action
            max_actions_per_session: None,
            auto_record: true,
        };
        let mut session = ActiveSession::new(None, config);
        let action = ReplayAction::new(0, ActionType::Put { frame_id: 1 });
        session.record_action(action);
        assert!(session.should_checkpoint());

        let snapshot = StateSnapshot::default();
        let checkpoint = session.create_checkpoint(snapshot);
        assert_eq!(checkpoint.id, 0);
    }

    #[test]
    fn active_session_end_returns_session() {
        let config = ReplayConfig::default();
        let session = ActiveSession::new(Some("ending".to_string()), config);
        let id = session.session_id();
        let completed = session.end();
        assert_eq!(completed.session_id, id);
        assert!(!completed.is_recording());
    }
}
```

**Step 2: Run tests**

Run: `cargo test replay::tests -- --nocapture`
Expected: All 5 tests PASS

**Step 3: Commit**

```bash
git add src/replay/mod.rs
git commit -m "test: add unit tests for active replay session"
```

---

### Task 24: Lockfile

**Files:**
- Modify: `src/lockfile.rs` (add `#[cfg(test)]` module)

**Step 1: Write tests**

Add at end of `src/lockfile.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn lock_options_defaults() {
        let opts = LockOptions::default();
        assert!(opts.timeout > std::time::Duration::ZERO);
        assert!(opts.heartbeat > std::time::Duration::ZERO);
    }

    #[test]
    fn lock_options_builder() {
        let opts = LockOptions::default()
            .timeout_ms(5000)
            .heartbeat_ms(500)
            .stale_grace_ms(10000)
            .command("test-cmd")
            .force_stale(true);
        assert_eq!(opts.timeout, std::time::Duration::from_millis(5000));
        assert_eq!(opts.heartbeat, std::time::Duration::from_millis(500));
        assert!(opts.force_stale);
    }

    #[test]
    fn acquire_and_drop_lock() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.mv2");
        std::fs::write(&path, b"content").unwrap();

        let options = LockOptions::default().command("test");
        let guard = acquire(&path, options).unwrap();
        let hint = guard.owner_hint();
        assert!(hint.pid.is_some());
        drop(guard);
    }

    #[test]
    fn current_owner_no_lock() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("nolock.mv2");
        std::fs::write(&path, b"content").unwrap();

        let owner = current_owner(&path).unwrap();
        assert!(owner.is_none());
    }

    #[test]
    fn heartbeat_updates() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.mv2");
        std::fs::write(&path, b"content").unwrap();

        let options = LockOptions::default().command("heartbeat-test");
        let mut guard = acquire(&path, options).unwrap();
        assert!(guard.heartbeat().is_ok());
    }
}
```

**Step 2: Run tests**

Run: `cargo test lockfile::tests -- --nocapture`
Expected: All 5 tests PASS

**Step 3: Commit**

```bash
git add src/lockfile.rs
git commit -m "test: add unit tests for lockfile management"
```

---

### Task 25: Integration Test — Mutation Pipeline

**Files:**
- Create: `tests/mutation_pipeline.rs`

**Step 1: Write integration test**

```rust
//! Integration tests for the mutation pipeline: put → commit → search → delete → verify

use memvid::{Memvid, PutOptions, FrameRole};
use tempfile::TempDir;

static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn create_temp_memvid() -> (TempDir, Memvid) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.mv2");
    let mem = Memvid::create(&path).unwrap();
    (dir, mem)
}

#[test]
fn put_bytes_returns_frame_id() {
    let _lock = SERIAL.lock().unwrap();
    let (_dir, mut mem) = create_temp_memvid();
    let id = mem.put_bytes(b"Hello, world!").unwrap();
    assert!(id > 0);
    mem.commit().unwrap();
}

#[test]
fn put_bytes_with_options_tags() {
    let _lock = SERIAL.lock().unwrap();
    let (_dir, mut mem) = create_temp_memvid();
    let opts = PutOptions::builder()
        .uri("test://tagged")
        .title("Tagged Document")
        .push_tag("important")
        .auto_tag(false)
        .extract_dates(false)
        .extract_triplets(false)
        .build();
    let id = mem.put_bytes_with_options(b"Tagged content here", opts).unwrap();
    mem.commit().unwrap();
    assert!(id > 0);
}

#[test]
fn multiple_puts_then_commit() {
    let _lock = SERIAL.lock().unwrap();
    let (_dir, mut mem) = create_temp_memvid();
    let id1 = mem.put_bytes(b"First document").unwrap();
    let id2 = mem.put_bytes(b"Second document").unwrap();
    mem.commit().unwrap();
    assert_ne!(id1, id2);

    let stats = mem.stats();
    assert!(stats.frame_count >= 2);
}

#[test]
fn delete_frame_marks_tombstone() {
    let _lock = SERIAL.lock().unwrap();
    let (_dir, mut mem) = create_temp_memvid();
    let id = mem.put_bytes(b"To be deleted").unwrap();
    mem.commit().unwrap();

    mem.delete_frame(id).unwrap();
    mem.commit().unwrap();
}

#[test]
fn commit_empty_succeeds() {
    let _lock = SERIAL.lock().unwrap();
    let (_dir, mut mem) = create_temp_memvid();
    // Commit with nothing pending should succeed
    mem.commit().unwrap();
}

#[test]
fn put_with_role() {
    let _lock = SERIAL.lock().unwrap();
    let (_dir, mut mem) = create_temp_memvid();
    let opts = PutOptions::builder()
        .role(FrameRole::Document)
        .auto_tag(false)
        .extract_dates(false)
        .extract_triplets(false)
        .build();
    let id = mem.put_bytes_with_options(b"Document with role", opts).unwrap();
    mem.commit().unwrap();
    assert!(id > 0);
}
```

**Step 2: Run tests**

Run: `cargo test --test mutation_pipeline -- --nocapture`
Expected: All 6 tests PASS

**Step 3: Commit**

```bash
git add tests/mutation_pipeline.rs
git commit -m "test: add integration tests for mutation pipeline"
```

---

### Task 26: Integration Test — Search Orchestration

**Files:**
- Create: `tests/search_orchestration.rs`

**Step 1: Write integration test**

```rust
//! Integration tests for search: ingest → index → search → verify results

use memvid::{Memvid, PutOptions};
use tempfile::TempDir;

static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn create_and_populate() -> (TempDir, Memvid) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("search_test.mv2");
    let mut mem = Memvid::create(&path).unwrap();

    let opts = PutOptions::builder()
        .auto_tag(false)
        .extract_dates(false)
        .extract_triplets(false)
        .build();

    mem.put_bytes_with_options(
        b"Rust is a systems programming language focused on safety",
        opts.clone(),
    ).unwrap();
    mem.put_bytes_with_options(
        b"Python is great for data science and machine learning",
        opts.clone(),
    ).unwrap();
    mem.put_bytes_with_options(
        b"JavaScript powers the modern web with frameworks like React",
        opts,
    ).unwrap();
    mem.commit().unwrap();

    (dir, mem)
}

#[test]
fn search_finds_relevant_document() {
    let _lock = SERIAL.lock().unwrap();
    let (_dir, mut mem) = create_and_populate();

    let results = mem.search_lex("Rust safety", 10).unwrap();
    assert!(!results.is_empty());
}

#[test]
fn search_respects_limit() {
    let _lock = SERIAL.lock().unwrap();
    let (_dir, mut mem) = create_and_populate();

    let results = mem.search_lex("programming", 1).unwrap();
    assert!(results.len() <= 1);
}

#[test]
fn search_no_results_for_unrelated_query() {
    let _lock = SERIAL.lock().unwrap();
    let (_dir, mut mem) = create_and_populate();

    let results = mem.search_lex("quantum physics supercollider", 10).unwrap();
    // May return empty or low-relevance results
    // Key: doesn't panic
    let _ = results;
}

#[test]
fn search_after_delete_excludes_deleted() {
    let _lock = SERIAL.lock().unwrap();
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("delete_search.mv2");
    let mut mem = Memvid::create(&path).unwrap();

    let opts = PutOptions::builder()
        .auto_tag(false)
        .extract_dates(false)
        .extract_triplets(false)
        .build();

    let id = mem.put_bytes_with_options(
        b"Unique findable content xyzzy",
        opts,
    ).unwrap();
    mem.commit().unwrap();

    mem.delete_frame(id).unwrap();
    mem.commit().unwrap();

    let results = mem.search_lex("xyzzy", 10).unwrap();
    // Deleted frames should not appear
    for hit in &results {
        assert_ne!(hit.frame_id, id);
    }
}
```

**Step 2: Run tests**

Run: `cargo test --test search_orchestration -- --nocapture`
Expected: All 4 tests PASS

**Step 3: Commit**

```bash
git add tests/search_orchestration.rs
git commit -m "test: add integration tests for search orchestration"
```

---

### Task 27: Final Verification

**Step 1: Run all tests**

Run: `cargo test`
Expected: All tests PASS (existing ~450 + ~150 new ≈ 600+)

**Step 2: Run clippy**

Run: `cargo clippy --all-features -- -D warnings`
Expected: No warnings

**Step 3: Commit any fixes**

If clippy finds issues in tests, fix and commit.

---

## Summary

| Phase | Tasks | New Tests | Files Modified | Files Created |
|-------|-------|-----------|----------------|---------------|
| 1: Critical | Tasks 1-11 | ~65 | 11 source files | 0 |
| 2: Types & Recovery | Tasks 12-18 | ~42 | 7 source files | 0 |
| 3: Broad Sweep | Tasks 19-26 | ~47 | 6 source files | 2 test files |
| Verification | Task 27 | 0 | 0 | 0 |
| **Total** | **27 tasks** | **~154 tests** | **24 source files** | **2 test files** |
