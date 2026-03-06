# Research: Code Review Fixes

**Feature Branch**: `001-code-review-fixes`
**Date**: 2026-03-05

## R1: Stable Hash Function for Node IDs (C1)

**Decision**: Use BLAKE3 truncated to u64 for `compute_node_id`
**Rationale**: BLAKE3 is already a dependency (v1.5.1), is cryptographically stable across compiler versions, and is faster than SipHash for short inputs. Truncating the 256-bit output to u64 via `hash.as_bytes()[..8]` with `u64::from_le_bytes` provides sufficient collision resistance for entity graph node IDs.
**Alternatives considered**:
- SipHash (stable but requires new dependency `siphasher`)
- FNV (stable, fast, but poor collision resistance)
- FxHash (not guaranteed stable across versions)

## R2: Auto-Migration Strategy for Old Hashes (C1 + FR-002)

**Decision**: Detect old-format hashes on open by sampling; recompute and persist on next commit
**Rationale**: The LogicMesh stores node IDs as u64 keys in a HashMap. On open, `needs_migration()` samples up to 10 nodes — it rehashes each node's canonical name with BLAKE3 and compares to the stored ID. If any mismatch is found, the mesh uses old-format hashes. This is O(1) amortized (bounded sample size). `migrate_node_ids()` then rehashes all entities and rebuilds the node map. Changes persist when the user next calls `commit()`.
**Alternatives considered**:
- Full rehash of all nodes for detection (O(n), expensive for large meshes)
- Dual-hash lookup at runtime (adds permanent overhead)
- Explicit migration API (burdens users)
- Version flag in header (requires header format change)

## R3: Staging Snapshot Struct (C3)

**Decision**: Create `MemvidSnapshot` struct that captures all mutable fields before staging operations
**Rationale**: The current `with_staging_lock` saves/restores only a subset of fields. A dedicated snapshot struct is explicit, auditable, and ensures new fields added in the future are captured by failing to compile if omitted (using struct destructuring).
**Alternatives considered**:
- Clone the entire Memvid struct (too expensive — includes file handles and indices)
- Manual field-by-field save/restore (current approach — error-prone)

## R4: SIMD Dimension Validation (H2)

**Decision**: Replace `debug_assert_eq!` with a runtime check that returns `Result<f32, MemvidError>` for dimension mismatches
**Rationale**: `debug_assert_eq!` is stripped in release builds, allowing mismatched dimensions to produce garbage results or undefined behavior. A proper check adds negligible overhead (single comparison) but prevents silent corruption.
**Alternatives considered**:
- `assert_eq!` (panics instead of returning error — not idiomatic for a library)
- Compile-time dimension checks via const generics (too invasive for current API)

## R5: XLSX Size Limits (H7)

**Decision**: Add `XLSX_MAX_FILE_BYTES = 100 MB` and `XLSX_MAX_ENTRY_BYTES = 1 GB` constants; check compressed size before opening, track decompressed bytes per entry
**Rationale**: Per clarification session, 100 MB / 1 GB limits balance usability with DoS protection. Check compressed size via `fs::metadata` before `ZipArchive::new`, and track cumulative decompressed bytes per entry during extraction.
**Alternatives considered**:
- Configurable limits via builder (deferred — can be added later)
- No limits with documentation warning (insufficient for untrusted input)

## R6: Whisper KV Cache Fix (H10)

**Decision**: Set `flush_kv_cache = false` after first token and pass only new tokens to decoder
**Rationale**: The current code passes all tokens on every step with `flush_kv_cache = true`, defeating the cache and causing O(n^2) complexity. Candle's Whisper decoder does support KV caching — the existing comment is incorrect. Pass only the last token after the first step.
**Alternatives considered**:
- Batched decoding (more complex, marginal benefit for typical audio lengths)
- External KV cache management (unnecessary — candle handles it internally)

## R7: API Key Zeroization (M8)

**Decision**: Use the `secrecy` crate's `SecretString` type for API keys
**Rationale**: `secrecy` provides `Zeroize`-on-drop semantics and prevents accidental logging via `Debug`/`Display` trait implementations. It's a lightweight dependency (~200 lines) with no transitive dependencies beyond `zeroize`.
**Alternatives considered**:
- Manual `zeroize` calls (error-prone, easy to miss)
- Custom wrapper type (reinvents `secrecy`)

## R8: WAL Checked Arithmetic (FR-016)

**Decision**: Use `checked_add` for WAL sequence number increments, return `MemvidError::WalOverflow` on overflow
**Rationale**: Per clarification session, checked arithmetic surfaces bugs early. WAL sequence numbers are u64 so overflow is practically impossible, but explicit handling prevents silent wrapping in edge cases.
**Alternatives considered**:
- Wrapping arithmetic (silent, practically safe but inconsistent with data integrity goals)
- Saturating arithmetic (hides the overflow condition)

## R9: Logging Unification (M20)

**Decision**: Replace all `use log::*` with `tracing` equivalents; remove `log` from Cargo.toml dependencies
**Rationale**: `tracing` is already the primary logging framework. The `tracing` macros (`info!`, `warn!`, `error!`) are API-compatible with `log` — migration is a find-and-replace operation. Three files affected: `extract.rs`, `mutation.rs`, `search/tantivy.rs`.
**Alternatives considered**:
- Keep both with `tracing-log` bridge (adds unnecessary complexity)
- Switch everything to `log` (loses structured span context)

## R10: Dead Code Removal (M22-M24)

**Decision**: Remove `publish_lex_delta`, `publish_vec_delta`, `publish_time_delta`, `publish_temporal_delta` from `segments.rs`, and `_entity_count` from `memory.rs`
**Rationale**: Per spec assumptions, these are internal functions with no backwards-compatibility concerns. Removing them reduces maintenance burden and eliminates `#[allow(dead_code)]` annotations.
**Alternatives considered**:
- Mark as `#[deprecated]` (unnecessary for internal code)
- Keep behind feature flag (over-engineering)
