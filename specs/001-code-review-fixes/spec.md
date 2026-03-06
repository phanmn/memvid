# Feature Specification: Code Review Fixes

**Feature Branch**: `001-code-review-fixes`
**Created**: 2026-03-05
**Status**: Draft
**Input**: User description: "code review fixes"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Data Integrity Protection (Priority: P1)

A developer uses memvid to store and retrieve documents in `.mv2` files. They upgrade their Rust toolchain and reopen an existing file. All previously stored data, node IDs, search indices, and temporal tracks must remain consistent and accessible without silent corruption.

**Why this priority**: Data corruption is the highest-severity class of bug. Unstable hashes (C1), incomplete rollback (C3), wrong frame IDs (H1), and truncated flags (H4) all risk silently corrupting persisted state.

**Independent Test**: Can be fully tested by creating an `.mv2` file, writing data with known content, then verifying all reads, searches, and mesh traversals return correct results after applying fixes.

**Acceptance Scenarios**:

1. **Given** an `.mv2` file created before the fix, **When** a developer opens it with the fixed library, **Then** all node IDs, frame references, and search results remain valid
2. **Given** a staging operation that fails mid-way, **When** the error is caught, **Then** all mutable fields are restored to their pre-staging values with no partial corruption
3. **Given** content is added via `put_bytes` but not yet committed, **When** a search is performed, **Then** any returned frame IDs are valid and resolvable (not WAL sequence numbers)
4. **Given** temporal track flags with values exceeding 16-bit range, **When** flags are written and read back, **Then** no silent truncation occurs — either full values are preserved or an explicit error is raised

---

### User Story 2 - Security and Resource Safety (Priority: P2)

A developer processes untrusted files (XLSX, PDF) through memvid's ingestion pipeline. The system must prevent resource exhaustion attacks (zip bombs, oversized files) and protect sensitive credentials (API keys) from leaking through insecure channels.

**Why this priority**: Security issues (H7, M8, M9) can lead to denial of service or credential exposure. These are externally exploitable and must be addressed before any public-facing deployment.

**Independent Test**: Can be tested by providing oversized or malicious XLSX files and verifying rejection, and by confirming API keys are not transmitted over unencrypted connections.

**Acceptance Scenarios**:

1. **Given** an XLSX file exceeding size limits, **When** ingested, **Then** the system rejects it with a clear error message before decompressing
2. **Given** an XLSX zip entry that would decompress to excessive size, **When** processed, **Then** the system rejects the entry with a size limit error
3. **Given** an API base URL using HTTP (not HTTPS), **When** configuring the embedder, **Then** the system warns or rejects the insecure URL (except localhost)
4. **Given** API credentials stored in memory, **When** the embedder is dropped, **Then** the key material is zeroized

---

### User Story 3 - Correctness in Release Builds (Priority: P3)

A developer builds memvid in release mode and runs vector similarity searches. The SIMD distance functions, CLIP embedding pipelines, and index lookups must behave correctly — no silent computation of garbage distances, no panics from out-of-bounds access, and no wrong-dimension embeddings entering the index.

**Why this priority**: Release-only bugs are insidious because they pass all tests but fail in production. SIMD bounds checks (H2), CLIP dimension validation (H3), and HNSW panic prevention (M11) are all release-mode correctness issues.

**Independent Test**: Can be tested by running vector operations with mismatched dimensions and corrupted indices in release mode, verifying proper errors instead of panics or garbage.

**Acceptance Scenarios**:

1. **Given** two vectors of different dimensions, **When** cosine similarity is computed in release mode, **Then** the system returns an error (not garbage or panic)
2. **Given** a CLIP model that returns unexpected tensor shape, **When** encoding completes, **Then** the system returns a dimension mismatch error
3. **Given** a corrupted HNSW index with out-of-bounds neighbor references, **When** search is performed, **Then** the system returns an error instead of panicking

---

### User Story 4 - Observability and Diagnostics (Priority: P4)

A developer encounters empty search results or degraded performance. The system must provide clear diagnostic information — logged warnings for index load failures, structured logging instead of println, and surfaced error states in stats output.

**Why this priority**: Without observability, developers cannot diagnose issues. Silent error swallowing (H8), mixed logging crates (M20), and println-based diagnostics (M21) make debugging unnecessarily difficult.

**Independent Test**: Can be tested by corrupting an index segment, opening the file, and verifying that warnings are logged and stats report the degraded state.

**Acceptance Scenarios**:

1. **Given** a corrupted lex index, **When** the file is opened, **Then** a warning is logged and `stats()` reports the load error
2. **Given** a `downgrade_to_shared` call with uncommitted changes, **When** called, **Then** the caller receives clear indication that the operation was not performed
3. **Given** doctor repair operations, **When** executed, **Then** all output uses structured logging (tracing), not println

---

### User Story 5 - Code Quality and Maintainability (Priority: P5)

A contributor works on the memvid codebase. Dead code is removed, memory leaks are fixed, performance regressions are addressed, and the codebase follows consistent patterns (single logging crate, lazy regex compilation, proper encoding validation).

**Why this priority**: Code quality issues (H6 memory leak, H10 O(n^2) whisper, M17 regex recompilation, M22 dead code) accumulate technical debt. Fixing them improves long-term maintainability.

**Independent Test**: Can be tested by verifying no `.leak()` calls remain in non-test code, benchmarking whisper decoding, and confirming dead code removal compiles cleanly.

**Acceptance Scenarios**:

1. **Given** a document with many headings, **When** chunked, **Then** memory usage is bounded (no String::leak)
2. **Given** a 60-second audio clip, **When** transcribed, **Then** decoding time is linear (not quadratic) in token count
3. **Given** the codebase, **When** compiled, **Then** no `#[allow(dead_code)]` annotations exist for the removed functions

---

### Edge Cases

- What happens when an existing `.mv2` file contains logic mesh data with old DefaultHasher IDs? (Migration required)
- What happens when `with_staging_lock` fails after modifying feature-gated fields (clip, temporal)?
- What happens when a zip entry reports a small compressed size but decompresses to gigabytes?
- What happens when `whole_minutes()` returns a value outside i16 range for UTC offsets?
- What happens when `CanonicalEncoding::from_byte` receives an unknown encoding byte?
- What happens when HNSW search references a neighbor index beyond the ID array bounds?

## Requirements *(mandatory)*

### Functional Requirements

**Critical (C1-C3):**
- **FR-001**: System MUST use a stable, portable hash function for `compute_node_id` that produces identical output across Rust compiler versions
- **FR-002**: System MUST auto-migrate existing `.mv2` files with logic mesh data created using the old hasher — detect old hashes on open, recompute transparently using the stable hasher, and persist on next commit
- **FR-003**: System MUST restore ALL mutable fields (including `memories_track`, `logic_mesh`, `sketch_track`, `clip_enabled`, `clip_index`, `vec_enabled`, `vec_index`, `vec_model`, `vec_compression`, and feature-gated fields) when a staging operation fails
- **FR-004**: System MUST add test coverage for the mutation pipeline's critical paths (chunking decisions, frame ID assignment, rollback completeness, capacity enforcement, delete+vacuum, temporal mention extraction)

**High (H1-H13):**
- **FR-005**: System MUST ensure frame IDs used in instant indexing match the IDs assigned during `apply_records`, or defer indexing to commit time
- **FR-006**: System MUST validate vector dimension equality in SIMD distance functions in all build modes (not just debug)
- **FR-007**: System MUST validate CLIP embedding dimensions match expected model dimensions after inference
- **FR-008**: System MUST handle temporal track flags consistently — either preserve full u32 range on disk or enforce u16 at the API boundary
- **FR-009**: System MUST verify BLAKE3 checksums when reading time index tracks
- **FR-010**: System MUST NOT use `String::leak()` for runtime data — use owned `String` or `Arc<str>` instead
- **FR-011**: System MUST enforce size limits on XLSX file processing (100 MB maximum compressed file size, 1 GB maximum per-entry decompression size)
- **FR-012**: System MUST log index load errors at warning level and surface them in stats output
- **FR-013**: System MUST return a distinguishable result from `downgrade_to_shared` when uncommitted changes prevent the operation
- **FR-014**: System MUST fix Whisper decoder KV cache usage to achieve linear (not quadratic) decoding complexity
- **FR-015**: System MUST provide `Default` implementation for `SearchRequest` and handle feature-flag conditional fields without breaking struct construction

**Medium (M1-M25):**
- **FR-016**: System MUST use checked arithmetic for WAL sequence numbers with explicit error on overflow
- **FR-017**: System MUST include model name in embedding cache keys to prevent cross-model collisions
- **FR-018**: System MUST return an error for unknown encoding bytes in `CanonicalEncoding::from_byte`
- **FR-019**: System MUST return an error (not panic) when HNSW search encounters out-of-bounds neighbor indices
- **FR-020**: System MUST use a single logging crate (`tracing`) throughout — remove all `log` crate usage
- **FR-021**: System MUST replace `catch_unwind(AssertUnwindSafe(...))` with proper error handling for vec index decoding
- **FR-022**: System MUST use lazy-compiled regexes in `pdf_extractor.rs` instead of compiling on every call
- **FR-023**: System MUST handle UTF-8 BOM consistently in PDF magic detection
- **FR-024**: System MUST open read-only snapshots with write permission disabled
- **FR-025**: System MUST remove dead code (`publish_lex_delta`, `publish_vec_delta`, `publish_time_delta`, `publish_temporal_delta`, unused `_entity_count` field)
- **FR-026**: System SHOULD protect API keys from memory exposure using zeroization
- **FR-027**: System SHOULD warn via `tracing::warn` when API base URL does not use HTTPS (except localhost) and proceed — do not reject
- **FR-028**: System SHOULD make stderr suppression thread-safe by holding it under the ONNX session mutex
- **FR-029**: System SHOULD clamp `whole_minutes()` UTC offset values to i16 range with a logged warning
- **FR-030**: System SHOULD clamp byte offsets to u32 with a logged warning when overflow occurs
- **FR-031**: System SHOULD fix `excel_serial_to_iso` midnight boundary value handling
- **FR-032**: System SHOULD remove or use the ignored regex parameters in `collect_table_region`

### Key Entities

- **Memvid**: Core struct holding file state, indices, WAL, and configuration — central to rollback (C3) and dirty-state (H9) fixes
- **LogicMesh**: Graph structure with nodes and edges — affected by hash stability (C1) and encapsulation (M2)
- **SearchRequest**: Query configuration struct — needs Default/builder (H12/H13) and feature-flag safety
- **TemporalTrack**: Chronological index with flags — affected by truncation (H4) and checksum (H5) fixes
- **MemvidSnapshot**: New struct for complete state capture during staging operations (C3 fix)

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: All existing `.mv2` files with logic mesh data can be opened and migrated without data loss after the hash function change
- **SC-002**: 100% of staging rollback scenarios restore all mutable fields — verified by a test that checks every field after a forced failure
- **SC-003**: Mutation pipeline test coverage reaches at least 80% line coverage for the critical paths identified in C2
- **SC-004**: Zero panics occur when SIMD functions receive mismatched dimensions, HNSW encounters corrupted indices, or CLIP returns unexpected shapes — all produce typed errors
- **SC-005**: XLSX files exceeding defined size limits are rejected before decompression begins
- **SC-006**: All index load failures are visible in `stats()` output and logged at warning level
- **SC-007**: Whisper decoding of a 60-second audio clip completes in linear time relative to token count (at least 2x faster than current quadratic behavior for long sequences)
- **SC-008**: No instances of `String::leak()` remain in non-test production code
- **SC-009**: The `log` crate is fully replaced by `tracing` — `log` is removed from production dependencies
- **SC-010**: All dead code marked with `#[allow(dead_code)]` is either removed or actively used

## Clarifications

### Session 2026-03-05

- Q: What specific size limits should FR-011 enforce for XLSX file processing? → A: 100 MB maximum compressed file size / 1 GB maximum per-entry decompression size
- Q: How should migration for old DefaultHasher node IDs (FR-002) be triggered? → A: Auto-migrate on open — detect old hashes, recompute transparently, persist on next commit
- Q: Should WAL sequence number arithmetic (FR-016) use checked or wrapping? → A: Checked arithmetic with explicit error on overflow
- Q: Should non-HTTPS API base URLs (FR-027) be warned or rejected? → A: Warn via tracing::warn and proceed (allow HTTP if user insists)

## Assumptions

- BLAKE3 is the preferred stable hasher since it is already a dependency (Option A for C1)
- Option A (snapshot struct) is preferred for staging rollback (C3) as it is explicit and auditable
- Option A (defer indexing to commit) is preferred for H1 as it is simpler and eliminates the ID mismatch entirely
- Option B (enforce u16 at API level) is preferred for H4 since no current usage needs >16 bits of flags
- The `secrecy` crate is acceptable as a new dependency for API key zeroization (M8)
- Dead code removal (M22, M23, M24) can proceed without preserving backwards compatibility since these are internal functions
- The recommended execution order from `review-fixes.md` will be followed: C1 → H6 → H7 → C3 → H2 → H8 → C2 → H1 → H4 → H5 → remaining items
