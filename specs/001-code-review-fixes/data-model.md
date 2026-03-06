# Data Model: Code Review Fixes

**Feature Branch**: `001-code-review-fixes`
**Date**: 2026-03-05

## Modified Entities

### MemvidSnapshot (NEW)

Captures all mutable state before staging operations for atomic rollback (C3).

| Field | Type | Source | Notes |
|-------|------|--------|-------|
| frame_counter | u64 | Memvid | Current frame ID counter |
| memories_track | Option<MemoriesTrack> | Memvid | Cloned memory track state |
| logic_mesh | Option<LogicMesh> | Memvid | Cloned entity graph |
| sketch_track | Option<SketchTrack> | Memvid | Cloned sketch cache |
| clip_enabled | bool | Memvid | CLIP feature state |
| clip_index | Option<ClipIndex> | Memvid | Cloned CLIP index (feature-gated) |
| vec_enabled | bool | Memvid | Vec feature state |
| vec_index | Option<VecIndex> | Memvid | Cloned vec index (feature-gated) |
| vec_model | Option<String> | Memvid | Vec model name |
| vec_compression | Option<Compression> | Memvid | Vec compression setting |
| dirty | bool | Memvid | Dirty flag state |

**Lifecycle**: Created at start of `with_staging_lock`, consumed on error (restore all fields), dropped on success.

**Validation**: Struct must use exhaustive field list — adding a new mutable field to Memvid without adding it to MemvidSnapshot must cause a compile error (use destructuring pattern).

### LogicMesh (MODIFIED)

| Change | Field/Method | Before | After |
|--------|-------------|--------|-------|
| Modified | `compute_node_id()` | `DefaultHasher` | BLAKE3 truncated to u64 |
| Added | `migrate_node_ids()` | N/A | Rehashes all nodes with BLAKE3, rebuilds node map |
| Added | `needs_migration()` | N/A | Detects old-format hashes by rehashing and comparing |

**Migration invariant**: After migration, `compute_node_id(name, kind)` for every node must equal the node's key in the HashMap.

### SearchRequest (MODIFIED)

| Change | Field | Before | After |
|--------|-------|--------|-------|
| Added | `Default` impl | None | All fields have sensible defaults |
| Modified | Feature-gated fields | Break struct construction | Use `#[cfg]` on fields + `Default` for safe construction |

### TemporalTrack (MODIFIED)

| Change | Field | Before | After |
|--------|-------|--------|-------|
| Modified | `flags` | u32 silently truncated to u16 on write | u16 at API boundary with validation error if >u16::MAX |
| Modified | UTC offset `whole_minutes()` | Unclamped i32 | Clamped to i16 range with `tracing::warn` if value exceeds bounds (FR-029) |

### WAL Sequence Numbers (MODIFIED)

| Change | Operation | Before | After |
|--------|-----------|--------|-------|
| Modified | Increment | Mixed checked/wrapping | Checked arithmetic, returns `MemvidError::WalOverflow` |

## New Error Variants

| Variant | Context | Trigger |
|---------|---------|---------|
| `DimensionMismatch { expected, got }` | SIMD distance, CLIP encode | Vectors of different dimensions |
| `WalOverflow` | WAL sequence increment | u64 overflow (theoretical) |
| `UnknownEncoding(u8)` | CanonicalEncoding::from_byte | Unrecognized encoding byte |
| `IndexCorrupted(String)` | HNSW search | Out-of-bounds neighbor reference |
| `FileTooLarge { path, size, limit }` | XLSX ingestion | Compressed file >100 MB |
| `DecompressionTooLarge { entry, size, limit }` | XLSX ingestion | Entry decompresses >1 GB |
| `DowngradeBlocked` | downgrade_to_shared | Uncommitted changes present |

## New Constants

| Name | Value | Location | Purpose |
|------|-------|----------|---------|
| `XLSX_MAX_FILE_BYTES` | 104_857_600 (100 MB) | `reader/xlsx.rs` | Compressed XLSX size limit |
| `XLSX_MAX_ENTRY_BYTES` | 1_073_741_824 (1 GB) | `reader/xlsx.rs` | Per-entry decompression limit |

## New Dependencies

| Crate | Version | Purpose | Feature-gated |
|-------|---------|---------|---------------|
| `secrecy` | ^0.10 | API key zeroization | No (lightweight) |

## Removed Items

| Item | Location | Reason |
|------|----------|--------|
| `publish_lex_delta()` | `memvid/segments.rs` | Dead code (M22) |
| `publish_vec_delta()` | `memvid/segments.rs` | Dead code (M22) |
| `publish_time_delta()` | `memvid/segments.rs` | Dead code (M22) |
| `publish_temporal_delta()` | `memvid/segments.rs` | Dead code (M22) |
| `_entity_count` field | `memvid/memory.rs` | Unused field (M23) |
| `log` crate dependency | `Cargo.toml` | Replaced by `tracing` (M20) |
