# Test Coverage Design

## Goal

Add comprehensive automated tests to all untested modules in memvid, using a phased approach that prioritizes critical paths first then broadens to full coverage.

## Current State

- 498 existing tests across 71 source files and 11 integration test files
- 64 source files have zero inline tests
- ~52.6% file coverage

## Approach: Phased Tiers (A)

### Phase 1 — Critical Hardening (~30 tests)

#### 1.1 Encryption Module (`src/encryption/`)

Inline unit tests in capsule.rs, capsule_stream.rs, crypto.rs, types.rs:

- `encrypt_decrypt_round_trip` — data survives encrypt/decrypt cycle
- `wrong_password_fails` — decryption with wrong password returns error
- `empty_payload_round_trip` — zero-length data edge case
- `large_payload_round_trip` — multi-block data (>64KB)
- `key_derivation_deterministic` — same password + salt = same key
- `different_salts_different_keys` — salt uniqueness
- `capsule_header_serialization` — header round-trips through bincode
- `stream_encrypt_decrypt` — streaming API matches non-streaming
- `corrupted_ciphertext_fails` — tampered data returns error
- `iv_uniqueness` — each encryption produces unique IV

Integration test: `tests/encryption_unit.rs`

#### 1.2 Mutation Pipeline (`src/memvid/mutation.rs`)

Inline unit tests:

- `wal_entry_creation` — correct structure and checksums
- `wal_flush_and_recover` — entries written to WAL can be replayed
- `chunk_planning_small_doc` — single small doc = single chunk
- `chunk_planning_large_doc` — large doc = multiple chunks with correct boundaries
- `chunk_boundary_word_split` — chunks don't split mid-word
- `metadata_extraction_basic` — title, author, date extracted
- `temporal_tag_extraction` — date references produce temporal tags
- `mime_routing_pdf` — PDF bytes routed to PDF extractor
- `mime_routing_docx` — DOCX bytes routed to DOCX extractor
- `mime_routing_plain_text` — plain text passed through
- `delete_marks_frame_tombstone` — delete marks frame, doesn't erase
- `commit_empty_is_noop` — empty commit succeeds without writing
- `put_bytes_with_options_respects_tags` — custom tags appear on frame
- `duplicate_content_handling` — same content twice = two frames

Integration test: `tests/mutation_pipeline.rs`

#### 1.3 Search Engine — Tantivy (`src/search/tantivy/`) + Orchestration (`src/memvid/search/`)

Inline unit tests — Tantivy layer:

- `schema_fields_registered` — all expected fields exist
- `query_simple_term` — single word = correct query
- `query_phrase` — quoted phrase = phrase query
- `query_boolean_and` — `a AND b` = boolean AND
- `query_boolean_or` — `a OR b` = boolean OR
- `query_negation` — `-term` = must-not clause
- `query_wildcard` — `test*` = prefix/wildcard query
- `index_and_search_round_trip` — index doc, search, find
- `empty_index_returns_no_results` — empty index = empty results
- `storage_write_read_round_trip` — index persists and reloads

Inline unit tests — Orchestration:

- `search_request_builder_defaults` — sane defaults
- `search_request_builder_top_k` — top_k respected
- `time_filter_before_date` — filters frames after cutoff
- `time_filter_after_date` — filters frames before cutoff
- `time_filter_date_range` — only in-range frames
- `time_filter_no_temporal_data` — frames without timestamps pass through
- `fallback_lex_only_search` — works without vec index
- `helpers_format_results` — correct snippets and scores
- `helpers_dedup_results` — duplicate frames collapsed
- `api_validate_empty_query_rejected` — empty query = error

Integration test: `tests/search_orchestration.rs`

#### 1.4 Document Readers (`src/reader/`)

Inline unit tests:

- `registry_returns_correct_reader` — MIME type = correct reader
- `registry_unknown_mime_returns_error` — unrecognized MIME = error
- `passthrough_returns_input_unchanged` — text in = text out
- `pdf_extracts_text` — mock PDF = extracted text
- `pdf_empty_returns_empty` — empty PDF = empty string
- `docx_extracts_paragraphs` — mock DOCX = paragraph text
- `docx_preserves_whitespace` — line breaks preserved
- `pptx_extracts_slide_text` — mock PPTX = slide text
- `pptx_multiple_slides_ordered` — multi-slide = ordered text
- `xlsx_extracts_cell_values` — mock XLSX = cell content
- `xlsx_handles_empty_cells` — sparse spreadsheet = no panic

Integration test: `tests/reader_formats.rs`

### Phase 2 — Type Safety & Recovery (~25 tests)

#### 2.1 Type Serialization (`src/types/`)

- `manifest_serialize_round_trip` — bincode encode/decode
- `manifest_default_valid` — sane default values
- `frame_serialize_round_trip` — encode/decode
- `frame_with_all_fields_set` — no fields lost
- `options_builder_defaults` — correct defaults
- `options_builder_chaining` — all methods work
- `verification_result_variants` — all outcomes representable
- `temporal_query_date_parsing` — date strings parse correctly
- `search_request_serialize` — round-trip
- `metadata_merge` — two maps merge correctly
- `acl_permission_checks` — allow/deny logic
- `ticket_lifecycle` — creation/validation/expiry

#### 2.2 Doctor & Maintenance (`src/memvid/doctor.rs`, `src/memvid/maintenance.rs`)

- `doctor_detect_truncated_header` — truncated file = corrupt
- `doctor_detect_bad_checksum` — mismatched checksum flagged
- `doctor_repair_wal_replay` — uncommitted WAL recovered
- `doctor_healthy_file_no_changes` — clean file passes
- `maintenance_compaction_removes_tombstones` — deleted frames removed
- `maintenance_compaction_preserves_live_data` — live frames survive

Integration test: `tests/type_safety.rs`

### Phase 3 — Broad Sweep (~20 tests)

#### 3.1 Extract & Analysis

- `extract_routes_by_mime` — correct extractor per MIME
- `extract_unsupported_mime_errors` — unknown format = error
- `temporal_parse_iso_date` — ISO date parsed
- `temporal_parse_relative` — "yesterday" resolved
- `temporal_parse_natural_language` — "January 15th" parsed
- `temporal_no_date_returns_none` — non-date = None

#### 3.2 Parallel Processing

- `planner_single_segment` — small input = one segment
- `planner_multiple_segments` — large input = correct partitions
- `builder_produces_valid_segments` — correct checksums
- `workers_pool_executes_tasks` — tasks dispatched and collected

#### 3.3 Infrastructure

- `registry_register_and_lookup` — register/lookup succeeds
- `registry_duplicate_errors` — double registration = error
- `lockfile_acquire_release` — lock/release/re-acquire
- `lockfile_double_acquire_fails` — second lock fails
- `error_display_messages` — readable messages
- `header_serialize_round_trip` — 4KB header write/read
- `header_magic_bytes_correct` — correct magic bytes
- `time_index_insert_and_lookup` — timestamps indexed
- `time_index_range_query` — range returns correct entries
- `temporal_index_round_trip` — persists and reloads
- `manifest_wal_append_and_replay` — append then replay

#### 3.4 Replay

- `replay_engine_empty_file` — empty = no events
- `replay_event_ordering` — chronological order
- `replay_types_serialize` — event types round-trip

## Constraints

- External services (CLIP, Whisper, API embeddings) will be mocked/stubbed
- Tests must pass without network access or ML model downloads
- Use `tempfile::TempDir` for file-based tests
- Follow existing patterns: `#[cfg(test)]` inline modules, `SERIAL_TEST_MUTEX` for file tests

## Expected Outcome

- ~75 new inline unit tests across 64 previously untested files
- ~5 new integration test files
- Coverage target: 80%+ file coverage (up from ~52.6%)
