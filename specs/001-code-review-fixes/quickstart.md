# Quickstart: Code Review Fixes

**Branch**: `001-code-review-fixes`

## Execution Order

Follow the recommended order from the review document. Each phase groups related fixes to minimize rework.

### Phase 1: Data Integrity (Critical)

```bash
# C1: Replace DefaultHasher with BLAKE3 in logic_mesh.rs
# Add auto-migration on open
cargo test --test lifecycle

# H6: Remove String::leak in chunker.rs (if present)
cargo test --test single_file

# C3: Implement MemvidSnapshot for staging rollback
# Covers all mutable fields including feature-gated ones
cargo test --test mutation

# C2: Add mutation pipeline tests
cargo test --test mutation_pipeline
```

### Phase 2: Security & Safety

```bash
# H7: Add XLSX size limits (100 MB / 1 GB)
cargo test --test xlsx_structured

# H2: SIMD dimension validation in release mode
cargo test --release -- simd

# M8: API key zeroization with secrecy crate
# M9: HTTPS warning for API URLs
cargo test -- api_embed
```

### Phase 3: Correctness

```bash
# H1: Defer instant indexing to commit time
# H4: Enforce u16 flags at API boundary
# H5: Verify BLAKE3 checksums on time index read
cargo test --test lifecycle --test search

# H3: CLIP dimension validation
cargo test -- clip  # requires vec feature

# M11: HNSW bounds checking
cargo test -- vec   # requires vec feature
```

### Phase 4: Observability

```bash
# H8: Log index load errors, surface in stats
# H9: downgrade_to_shared returns DowngradeBlocked
# M20: Replace log crate with tracing
# M21: Replace println! with tracing in doctor.rs
cargo test
```

### Phase 5: Code Quality

```bash
# H10: Fix Whisper KV cache (O(n^2) -> O(n))
cargo test -- whisper  # requires whisper feature

# M22-M24: Remove dead code
# M17: Lazy regex compilation
# M18: UTF-8 BOM handling
# Remaining medium-priority items
cargo test
cargo clippy
```

## Verification

```bash
# Full test suite
cargo test

# Release mode (catches H2, H3 regressions)
cargo test --release

# Clippy (catches dead code, unused imports)
cargo clippy -- -D warnings

# Verify no String::leak in non-test code
grep -rn "\.leak()" src/ --include="*.rs" | grep -v test | grep -v example

# Verify no log crate usage
grep -rn "use log::" src/ --include="*.rs"

# Verify no println! in production code (excluding doctor macro, tests, examples)
grep -rn "println!" src/ --include="*.rs" | grep -v test | grep -v example | grep -v "macro_rules"
```

## Key Files to Modify

| File | Changes | Priority |
|------|---------|----------|
| `src/types/logic_mesh.rs` | BLAKE3 hash, migration | C1 |
| `src/memvid/mutation.rs` | MemvidSnapshot, staging rollback, checked WAL arithmetic | C2, C3 |
| `src/simd.rs` | Runtime dimension check | H2 |
| `src/clip.rs` | Dimension validation | H3 |
| `src/reader/xlsx.rs` | Size limits | H7 |
| `src/whisper.rs` | KV cache fix | H10 |
| `src/memvid/search/api.rs` | Log index errors, stats | H8 |
| `src/structure/chunker.rs` | Remove String::leak | H6 |
| `src/api_embed.rs` | SecretString, HTTPS warn | M8, M9 |
| `src/memvid/segments.rs` | Remove dead code | M22 |
| `src/memvid/memory.rs` | Remove _entity_count | M23 |
| `src/memvid/doctor.rs` | tracing instead of println | M21 |
| `src/extract.rs` | tracing instead of log | M20 |
| `src/table/pdf_extractor.rs` | Lazy regex | M17 |
| `src/vec.rs` | Bounds checking, proper error handling | M11, M15 |
| `src/types/search.rs` | Default impl | H12, H13 |
| `src/io/time_index.rs` | BLAKE3 checksum verification | H5 |
| `src/io/temporal_index.rs` | u16 flag enforcement | H4 |
| `src/types/common.rs` | CanonicalEncoding error | M4 |
