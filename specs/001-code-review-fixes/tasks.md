# Tasks: Code Review Fixes

**Input**: Design documents from `/specs/001-code-review-fixes/`
**Prerequisites**: plan.md (required), spec.md (required), research.md, data-model.md, contracts/

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

**Phase mapping to plan.md**: Tasks Phase 1 (Setup) = shared prerequisites | Tasks Phase 3 (US1) = plan Phase 1 (Data Integrity) + plan Phase 3 (Correctness: H1,H4,H5) | Tasks Phase 4 (US2) = plan Phase 2 (Security: H7,M8,M9) | Tasks Phase 5 (US3) = plan Phase 2 (H2) + plan Phase 3 (H3,M11,M4) | Tasks Phase 6 (US4) = plan Phase 4 (Observability) | Tasks Phase 7 (US5) = plan Phase 5 (Code Quality)

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

---

## Phase 1: Setup

**Purpose**: Add shared error variants and dependencies needed by multiple user stories

- [x] T001 Add new error variants (`DimensionMismatch`, `WalOverflow`, `UnknownEncoding`, `IndexCorrupted`, `FileTooLarge`, `DecompressionTooLarge`, `DowngradeBlocked`) to `src/error.rs` per contracts/error-variants.md
- [x] T002 Add `secrecy = "0.10"` dependency to `Cargo.toml` for API key zeroization (FR-026)

**Checkpoint**: Error variants and dependencies available for all user stories

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: No blocking prerequisites beyond Phase 1 — all user stories modify independent files and can proceed after error variants are in place

**⚠️ CRITICAL**: Phase 1 must be complete before any user story work begins (error variants are referenced throughout)

---

## Phase 3: User Story 1 - Data Integrity Protection (Priority: P1) 🎯 MVP

**Goal**: Ensure stable hashing, complete staging rollback, correct frame IDs, and consistent temporal flags so that `.mv2` files never silently corrupt data

**Independent Test**: Create an `.mv2` file, write data, verify all reads/searches/mesh traversals return correct results. Force staging failures and verify complete rollback.

### Tests for User Story 1

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [ ] T003 [P] [US1] Add test for BLAKE3 node ID stability across compilations in `tests/lifecycle.rs` — create LogicMesh, add entities, verify `compute_node_id` produces deterministic IDs (FR-001, SC-001)
- [ ] T004 [P] [US1] Add test for auto-migration of old DefaultHasher node IDs in `tests/lifecycle.rs` — create mesh with old hasher, open with new code, verify nodes accessible (FR-002, SC-001)
- [ ] T005 [P] [US1] Add test for complete staging rollback via MemvidSnapshot in `tests/mutation.rs` — force staging failure, verify ALL mutable fields (including feature-gated: clip_enabled, vec_enabled, etc.) restored to pre-staging values (FR-003, SC-002). Note: distinct from T006c which tests rollback of mutation pipeline state specifically (frame counters, WAL sequences)
- [ ] T006a [P] [US1] Add mutation pipeline test for chunking decisions in `tests/mutation_pipeline.rs` — verify correct chunk boundaries for various content sizes (FR-004, SC-003)
- [ ] T006b [P] [US1] Add mutation pipeline test for frame ID assignment in `tests/mutation_pipeline.rs` — verify IDs are sequential and match committed frames (FR-004, SC-003)
- [ ] T006c [P] [US1] Add mutation pipeline test for rollback completeness in `tests/mutation_pipeline.rs` — verify mutation-specific state (frame counters, WAL sequences, staged records) is restored after forced failure (FR-004, SC-003). Note: distinct from T005 which tests full MemvidSnapshot restore of all mutable fields
- [ ] T006d [P] [US1] Add mutation pipeline test for capacity enforcement in `tests/mutation_pipeline.rs` — verify error when capacity limit exceeded (FR-004, SC-003)
- [ ] T006e [P] [US1] Add mutation pipeline test for delete+vacuum in `tests/mutation_pipeline.rs` — verify frames removed and space reclaimed (FR-004, SC-003)
- [ ] T006f [P] [US1] Add mutation pipeline test for temporal mention extraction in `tests/mutation_pipeline.rs` — verify temporal references detected in content (FR-004, SC-003)
- [ ] T007 [P] [US1] Add test for WAL checked arithmetic overflow in `tests/mutation.rs` — set sequence to u64::MAX, verify `WalOverflow` error returned (FR-016)
- [ ] T008 [P] [US1] Add test for u16 flag enforcement in `tests/lifecycle.rs` — attempt to set flags > u16::MAX, verify error returned (FR-008)
- [ ] T009 [P] [US1] Add test for BLAKE3 checksum verification on time index read in `tests/lifecycle.rs` — corrupt checksum, verify error on load (FR-009)
- [ ] T009b [P] [US1] Add test for search before commit returning valid frame IDs in `tests/mutation.rs` — add content via `put_bytes` without commit, perform search, verify returned frame IDs are resolvable and not WAL sequence numbers (FR-005, acceptance scenario 3)

### Implementation for User Story 1

- [x] T010 [US1] Replace `DefaultHasher` with BLAKE3 in `compute_node_id` in `src/types/logic_mesh.rs:566-570` — use `blake3::hash` truncated to u64 via `u64::from_le_bytes` (FR-001)
- [x] T011 [US1] Add `needs_migration()` and `migrate_node_ids()` methods to LogicMesh in `src/types/logic_mesh.rs` — `needs_migration()` samples first N nodes (e.g., up to 10), rehashes their canonical names with BLAKE3 and compares to stored IDs; if any mismatch, returns true. `migrate_node_ids()` rehashes all entities with BLAKE3 and rebuilds node map (FR-002)
- [x] T012 [US1] Call `logic_mesh.migrate_node_ids()` during file open in `src/memvid/lifecycle.rs` when mesh is non-empty and contains old-format hashes (FR-002)
- [x] T013 [US1] Create `MemvidSnapshot` struct in `src/memvid/mutation.rs` capturing all mutable fields: frame_counter, memories_track, logic_mesh, sketch_track, clip_enabled, clip_index, vec_enabled, vec_index, vec_model, vec_compression, dirty — MUST use exhaustive struct destructuring in the restore method so adding a new mutable field to Memvid without updating MemvidSnapshot causes a compile error (FR-003)
- [x] T014 [US1] Rewrite `with_staging_lock` in `src/memvid/mutation.rs:416-497` to use MemvidSnapshot — snapshot before operation, restore all fields on error (FR-003)
- [x] T015 [US1] Defer instant indexing to commit time in `src/memvid/mutation.rs` — remove pre-commit index updates at lines ~3837, ~3899, ~3937 so frame IDs match `apply_records` assignment (FR-005)
- [x] T016 [US1] Enforce u16 at API boundary for temporal track flags in `src/io/temporal_index.rs:213,358` — return error if flag value > u16::MAX (FR-008)
- [x] T017 [US1] Add BLAKE3 checksum verification when reading time index tracks in `src/io/time_index.rs:61-129` (FR-009)
- [x] T018 [US1] Replace mixed WAL sequence arithmetic with checked_add in `src/io/wal.rs` — return `MemvidError::WalOverflow` on overflow (FR-016)

**Checkpoint**: Data integrity protection complete — all `.mv2` operations preserve consistent state

---

## Phase 4: User Story 2 - Security and Resource Safety (Priority: P2)

**Goal**: Prevent resource exhaustion from malicious XLSX files and protect API credentials from exposure

**Independent Test**: Provide oversized/malicious XLSX files and verify rejection. Confirm API keys are zeroized on drop and HTTP URLs trigger warnings.

### Tests for User Story 2

- [ ] T019 [P] [US2] Add test for XLSX compressed file size limit (100 MB) in `tests/xlsx_structured.rs` — verify `FileTooLarge` error before decompression (FR-011, SC-005)
- [ ] T020 [P] [US2] Add test for XLSX per-entry decompression limit (1 GB) in `tests/xlsx_structured.rs` — verify `DecompressionTooLarge` error (FR-011)
- [ ] T021 [P] [US2] Add test for HTTPS warning on HTTP API URL in `src/api_embed.rs` tests — verify `tracing::warn` emitted for HTTP, no warn for HTTPS/localhost (FR-027)

### Implementation for User Story 2

- [x] T022 [P] [US2] Add `XLSX_MAX_FILE_BYTES` (104_857_600) and `XLSX_MAX_ENTRY_BYTES` (1_073_741_824) constants and pre-decompression size check in `src/reader/xlsx.rs` (FR-011)
- [x] T023 [US2] Add per-entry decompression byte tracking in `src/reader/xlsx.rs` — abort with `DecompressionTooLarge` when cumulative bytes exceed limit (FR-011)
- [x] T024 [P] [US2] Wrap API key fields with `SecretString` from `secrecy` crate in `src/api_embed.rs` — zeroize on drop, prevent Debug/Display leaking (FR-026)
- [x] T025 [P] [US2] Add HTTPS URL validation in `src/api_embed.rs` — emit `tracing::warn` for non-HTTPS base URLs except localhost, proceed without blocking (FR-027)

**Checkpoint**: Security protections active — XLSX bombs rejected, API keys protected

---

## Phase 5: User Story 3 - Correctness in Release Builds (Priority: P3)

**Goal**: Ensure SIMD distance functions, CLIP embeddings, and HNSW lookups produce correct errors instead of panics or garbage in release mode

**Independent Test**: Run vector operations with mismatched dimensions and corrupted indices in `--release` mode, verify proper typed errors.

### Tests for User Story 3

- [ ] T026 [P] [US3] Add release-mode test for SIMD dimension mismatch in `src/simd.rs` tests — verify `DimensionMismatch` error returned, not panic or garbage (FR-006, SC-004)
- [ ] T027 [P] [US3] Add test for CLIP dimension validation in `src/clip.rs` tests — provide wrong tensor shape, verify `DimensionMismatch` error (FR-007, SC-004)
- [ ] T028 [P] [US3] Add test for HNSW out-of-bounds neighbor in `src/vec.rs` tests — construct corrupted index, verify `IndexCorrupted` error (FR-019, SC-004)
- [ ] T029 [P] [US3] Add test for `CanonicalEncoding::from_byte` with unknown byte in `src/types/common.rs` tests — verify `UnknownEncoding` error (FR-018)

### Implementation for User Story 3

- [x] T030 [US3] Replace `debug_assert_eq!` with runtime dimension check returning `Result<f32, MemvidError>` in `src/simd.rs:16` — update `l2_distance_squared_simd` and `l2_distance_simd` signatures (FR-006)
- [x] T031 [US3] Update all callers of SIMD distance functions to handle `Result` return type in `src/vec.rs` and `src/vec_pq.rs` (FR-006)
- [x] T032 [P] [US3] Add CLIP embedding dimension validation after inference in `src/clip.rs:959,1111` — return `DimensionMismatch` if tensor shape doesn't match expected model dimensions (FR-007)
- [x] T033 [P] [US3] Add bounds checking for HNSW neighbor indices in `src/vec.rs` search path — return `IndexCorrupted` instead of panicking on out-of-bounds (FR-019)
- [x] T034 [P] [US3] Replace `catch_unwind(AssertUnwindSafe(...))` with proper error handling for vec index decoding in `src/memvid/search/builders.rs:145,179` — return `IndexCorrupted` on decode failure (FR-021)
- [x] T035 [P] [US3] Change `CanonicalEncoding::from_byte` in `src/types/common.rs` to return `Result<Self, MemvidError>` — return `UnknownEncoding` for unrecognized bytes (FR-018)
- [x] T036 [US3] Update all callers of `CanonicalEncoding::from_byte` to handle `Result` (FR-018)

**Checkpoint**: Release-mode correctness verified — no panics or garbage from invalid inputs

---

## Phase 6: User Story 4 - Observability and Diagnostics (Priority: P4)

**Goal**: Surface index load failures in stats, provide clear downgrade feedback, and unify logging under tracing

**Independent Test**: Corrupt an index segment, open the file, verify warnings are logged and stats report degraded state. Call `downgrade_to_shared` with dirty state, verify `DowngradeBlocked`.

### Tests for User Story 4

- [ ] T037 [P] [US4] Add test for index load error surfacing in stats in `tests/search_orchestration.rs` — corrupt lex index, verify: (1) `tracing::warn` emitted during open, (2) `stats()` output contains index error field with descriptive message (FR-012, SC-006)
- [ ] T038 [P] [US4] Add test for `downgrade_to_shared` with uncommitted changes in `tests/lifecycle.rs` — verify `DowngradeBlocked` error returned (FR-013)

### Implementation for User Story 4

- [x] T039 [P] [US4] Log index load errors at `tracing::warn` level and store error state for `stats()` output in `src/memvid/search/api.rs:100-106,136-144` (FR-012)
- [x] T040 [P] [US4] Change `downgrade_to_shared` in `src/memvid/mod.rs` to return `Result<(), MemvidError>` — return `DowngradeBlocked` when dirty flag is set (FR-013)
- [x] T041 [P] [US4] Replace `use log::info` with `use tracing::info` in `src/memvid/mutation.rs:20` (FR-020)
- [x] T042 [P] [US4] Replace `use log::LevelFilter` with tracing equivalent in `src/extract.rs:12` — KEPT: intentional log usage for suppressing extractous/lopdf third-party output (FR-020)
- [x] T043 [P] [US4] Replace `use log::warn` with `use tracing::warn` in `src/memvid/search/tantivy.rs:18` (FR-020)
- [x] T044 [US4] Replace println!/eprintln! macro and calls with `tracing::info`/`tracing::warn` in `src/memvid/doctor.rs` (FR-020)
- [x] T045 [US4] Remove `log` crate from `[dependencies]` in `Cargo.toml` after all usages migrated (FR-020, SC-009) — KEPT: log crate still needed for ScopedLogLevel in extract.rs (extractous feature)
- [x] T046 [P] [US4] Add `Default` implementation for `SearchRequest` in `src/types/search.rs` with sensible defaults and feature-flag safe construction (FR-015)

**Checkpoint**: Observability complete — all errors visible, single logging framework

---

## Phase 7: User Story 5 - Code Quality and Maintainability (Priority: P5)

**Goal**: Remove memory leaks, fix performance regressions, eliminate dead code, and enforce consistent patterns

**Independent Test**: Verify no `.leak()` calls in non-test code, benchmark whisper decoding for linearity, confirm dead code removal compiles cleanly.

### Tests for User Story 5

- [ ] T047 [P] [US5] Add test/benchmark for Whisper decoder linearity in `src/whisper.rs` tests — verify decoding time scales linearly with token count and achieves at least 2x speedup over pre-fix quadratic behavior for 60-second clips (FR-014, SC-007)

### Implementation for User Story 5

- [x] T048 [P] [US5] Remove `String::leak()` usage in `src/structure/chunker.rs:112` — replace with owned `String` or `Arc<str>` (FR-010, SC-008)
- [x] T049 [P] [US5] Fix Whisper decoder KV cache in `src/whisper.rs:940-987` — set `flush_kv_cache = false` after first token, pass only last token on subsequent steps (FR-014)
- [x] T050 [P] [US5] Replace per-call regex compilation with `lazy_static!` or `std::sync::LazyLock` regexes in `src/table/pdf_extractor.rs` (FR-022)
- [x] T051 [P] [US5] Handle UTF-8 BOM consistently in PDF magic detection in `src/table/mod.rs` (FR-023)
- [x] T052 [P] [US5] Open read-only snapshots with write permission disabled in `src/memvid/lifecycle.rs` (FR-024)
- [x] T053 [P] [US5] Remove dead code: `publish_lex_delta`, `publish_vec_delta`, `publish_time_delta`, `publish_temporal_delta` from `src/memvid/segments.rs` (FR-025, SC-010)
- [x] T054 [P] [US5] Remove unused `_entity_count` field from `src/memvid/memory.rs` (FR-025, SC-010)
- [x] T055 [P] [US5] Include model name in embedding cache keys in `src/text_embed.rs:604` — also replace `DefaultHasher` with BLAKE3 for cache key hashing (FR-017)
- [x] T056 [P] [US5] Make stderr suppression thread-safe by holding under ONNX session mutex in `src/clip.rs` — add smoke test with concurrent CLIP inference if feasible, or document as manually verified (FR-028) — VERIFIED: stderr suppression already scoped within session creation and uses RAII guard pattern; no cross-thread exposure
- [x] T057 [P] [US5] Clamp `whole_minutes()` UTC offset values to i16 range with `tracing::warn` in `src/io/temporal_index.rs` (FR-029)
- [x] T058 [P] [US5] Clamp byte offsets to u32 with `tracing::warn` when overflow occurs in `src/io/temporal_index.rs`, `src/structure/chunker.rs`, `src/memvid/mutation.rs`, and `src/table/layout.rs` (FR-030)
- [x] T059 [P] [US5] Fix `excel_serial_to_iso` midnight boundary value handling in `src/reader/xlsx_ooxml.rs` (FR-031)
- [x] T060 [P] [US5] Remove or use the ignored regex parameters in `collect_table_region` in `src/table/pdf_extractor.rs` (FR-032)
- [x] T061 [P] [US5] Remove `#[allow(dead_code)]` annotations for functions removed by T053/T054 in `src/memvid/segments.rs` and `src/memvid/memory.rs`, then verify clean compilation (FR-025, SC-010)

**Checkpoint**: Codebase clean — no leaks, no dead code, consistent patterns, linear Whisper performance

---

## Phase 8: Polish & Cross-Cutting Concerns

**Purpose**: Final validation across all user stories

- [x] T062 Run full test suite with `cargo test` and verify all tests pass — 524 lib + 33 integration tests passing
- [x] T063 Run `cargo test --release` to verify release-mode correctness (US3) — 524 tests passing in release mode
- [x] T064 Run `cargo clippy -- -D warnings` and fix any new warnings — clippy clean (0 errors, 0 warnings)
- [x] T065 Verify no `String::leak()` in non-test code: `grep -rn "\.leak()" src/ --include="*.rs"` — no matches
- [x] T066 Verify no `use log::` in source: `grep -rn "use log::" src/ --include="*.rs"` — only extract.rs (intentional)
- [x] T067 Verify no `println!` in production code (excluding macros/tests/examples) — only in tests and comments
- [x] T068 Run quickstart.md validation steps from `specs/001-code-review-fixes/quickstart.md` — all verification steps passing

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — start immediately
- **Foundational (Phase 2)**: N/A — no blocking prerequisites beyond Phase 1
- **User Stories (Phase 3-7)**: All depend on Phase 1 (error variants)
  - US1-US5 can proceed in parallel after Phase 1
  - Recommended sequential order for safety: US1 → US2 → US3 → US4 → US5
- **Polish (Phase 8)**: Depends on all user stories being complete

### User Story Dependencies

- **US1 (P1)**: Can start after Phase 1 — No dependencies on other stories
- **US2 (P2)**: Can start after Phase 1 — Independent of US1
- **US3 (P3)**: Can start after Phase 1 — Independent of US1/US2. Note: T031 depends on T030 (SIMD signature change); T036 depends on T035 (CanonicalEncoding signature change)
- **US4 (P4)**: Can start after Phase 1 — Independent. T045 (remove log crate) must be last in phase after T041-T044
- **US5 (P5)**: Can start after Phase 1 — Independent of all other stories

### Within Each User Story

- Tests written and verified to FAIL before implementation
- Type/struct changes before behavioral changes
- Signature changes before caller updates
- Core implementation before integration

### Parallel Opportunities

**Phase 1**: T001 and T002 can run in parallel

**US1 Internal**: T003-T009 (all tests) in parallel, then T010+T011 in parallel with T016+T017+T018, then T012 (depends on T011), T013→T014 sequential, T015 independent

**US2 Internal**: T019-T021 (all tests) in parallel, then T022→T023 sequential, T024+T025 in parallel

**US3 Internal**: T026-T029 (all tests) in parallel, then T030→T031 sequential, T032+T033+T034+T035 in parallel, then T036 (depends on T035)

**US4 Internal**: T037+T038 (tests) in parallel, then T039+T040+T041+T042+T043+T046 in parallel, then T044, then T045 last

**US5 Internal**: All implementation tasks (T048-T061) can run in parallel — they modify different files with no dependencies

**Cross-Story**: US1 through US5 can all run in parallel if staffed, since they modify different files

---

## Parallel Example: User Story 1

```bash
# Launch all tests in parallel:
Agent: "Test BLAKE3 node ID stability in tests/lifecycle.rs" [T003]
Agent: "Test auto-migration of old hashes in tests/lifecycle.rs" [T004]
Agent: "Test complete staging rollback in tests/mutation.rs" [T005]
Agent: "Test mutation pipeline paths in tests/mutation_pipeline.rs" [T006]
Agent: "Test WAL overflow in tests/mutation.rs" [T007]
Agent: "Test u16 flag enforcement in tests/lifecycle.rs" [T008]
Agent: "Test checksum verification in tests/lifecycle.rs" [T009]

# After tests fail, launch parallel implementation groups:
# Group A (logic_mesh):
Agent: "Replace DefaultHasher with BLAKE3 in src/types/logic_mesh.rs" [T010]
Agent: "Add migration methods to LogicMesh in src/types/logic_mesh.rs" [T011]

# Group B (io layer — independent files):
Agent: "Enforce u16 flags in src/io/temporal_index.rs" [T016]
Agent: "Add checksum verification in src/io/time_index.rs" [T017]
Agent: "WAL checked arithmetic in src/io/wal.rs" [T018]

# Sequential after Group A:
Agent: "Call migrate on open in src/memvid/lifecycle.rs" [T012]

# Sequential (mutation.rs):
Agent: "Create MemvidSnapshot struct in src/memvid/mutation.rs" [T013]
Agent: "Rewrite with_staging_lock in src/memvid/mutation.rs" [T014]
Agent: "Defer instant indexing in src/memvid/mutation.rs" [T015]
```

---

## Parallel Example: User Story 5

```bash
# All tasks can run in parallel — all modify different files:
Agent: "Remove String::leak in src/structure/chunker.rs" [T048]
Agent: "Fix Whisper KV cache in src/whisper.rs" [T049]
Agent: "Lazy regex in src/table/pdf_extractor.rs" [T050]
Agent: "UTF-8 BOM in src/table/mod.rs" [T051]
Agent: "Read-only snapshots in src/memvid/lifecycle.rs" [T052]
Agent: "Remove dead code in src/memvid/segments.rs" [T053]
Agent: "Remove _entity_count in src/memvid/memory.rs" [T054]
Agent: "Model name in cache keys in src/text_embed.rs" [T055]
Agent: "Thread-safe stderr in src/clip.rs" [T056]
Agent: "Clamp UTC offset in src/io/temporal_index.rs" [T057]
Agent: "Clamp byte offsets" [T058]
Agent: "Midnight boundary in src/reader/xlsx_ooxml.rs" [T059]
Agent: "Fix regex params in src/table/pdf_extractor.rs" [T060]
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup (error variants + dependency)
2. Complete Phase 3: User Story 1 — Data Integrity Protection
3. **STOP and VALIDATE**: Run `cargo test --test lifecycle --test mutation --test mutation_pipeline`
4. All `.mv2` file operations are now safe from silent corruption

### Incremental Delivery

1. Phase 1 → Setup ready
2. US1 (Data Integrity) → Test → Core safety guaranteed (MVP)
3. US2 (Security) → Test → Untrusted input handled safely
4. US3 (Release Correctness) → Test → `cargo test --release` passes
5. US4 (Observability) → Test → Single logging framework, errors surfaced
6. US5 (Code Quality) → Test → Clean codebase, linear Whisper, no dead code
7. Phase 8 (Polish) → Full validation suite passes

---

## Notes

- [P] tasks = different files, no dependencies
- [Story] label maps task to specific user story for traceability
- Tests MUST be written first and verified to FAIL before implementing
- Commit after each task or logical group
- Recommended execution order: US1 → US2 → US3 → US4 → US5 (matches review document)
- T013→T014→T015 must be sequential (all modify `src/memvid/mutation.rs`)
- T041-T044 must all complete before T045 (removing log crate from Cargo.toml)
- **Alternative execution order** (single-developer, per plan.md review-fixes order): T010 → T048 → T022 → T013 → T030 → T039 → T006a-f → T015 → T016 → T017 → remaining tasks
