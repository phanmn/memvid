# Error Variant Contracts

**Feature Branch**: `001-code-review-fixes`
**Date**: 2026-03-05

## New Error Variants for `MemvidError`

All new variants are added to the existing `MemvidError` enum in `src/error.rs`.

### DimensionMismatch

```rust
/// Vector dimensions do not match for distance computation or index insertion.
#[error("dimension mismatch: expected {expected}, got {got}")]
DimensionMismatch { expected: usize, got: usize },
```

**Producers**: `simd::l2_distance_squared_simd`, `simd::l2_distance_simd`, `clip::encode_image`, `clip::encode_text`
**Consumers**: `vec.rs` search paths, `clip.rs` encoding paths
**Test**: Provide two vectors of different lengths; assert `DimensionMismatch` is returned (not panic or garbage)

### WalOverflow

```rust
/// WAL sequence number overflow (practically unreachable with u64).
#[error("WAL sequence number overflow")]
WalOverflow,
```

**Producers**: `io::wal` sequence increment
**Consumers**: `memvid::mutation` write paths
**Test**: Unit test with sequence at `u64::MAX`; assert `WalOverflow` returned

### UnknownEncoding

```rust
/// Unrecognized encoding byte in frame metadata.
#[error("unknown encoding byte: {0:#04x}")]
UnknownEncoding(u8),
```

**Producers**: `types::common::CanonicalEncoding::from_byte`
**Consumers**: Frame deserialization paths
**Test**: Call `from_byte(0xFF)`; assert `UnknownEncoding(0xFF)` returned

### IndexCorrupted

```rust
/// Index contains invalid internal references.
#[error("corrupted index: {0}")]
IndexCorrupted(String),
```

**Producers**: `vec.rs` HNSW search (out-of-bounds neighbor), `memvid/search/builders.rs` index decode
**Consumers**: Search orchestration layer
**Test**: Construct index with neighbor ID beyond array bounds; assert `IndexCorrupted` returned

### FileTooLarge

```rust
/// Input file exceeds size limit.
#[error("file too large: {path} is {size} bytes (limit: {limit})")]
FileTooLarge { path: String, size: u64, limit: u64 },
```

**Producers**: `reader::xlsx` file size check
**Consumers**: Document ingestion pipeline
**Test**: Create file >100 MB; assert `FileTooLarge` returned before decompression

### DecompressionTooLarge

```rust
/// Decompressed entry exceeds size limit.
#[error("decompression limit exceeded for entry '{entry}': {size} bytes (limit: {limit})")]
DecompressionTooLarge { entry: String, size: u64, limit: u64 },
```

**Producers**: `reader::xlsx` per-entry decompression tracking
**Consumers**: Document ingestion pipeline
**Test**: Create ZIP with entry that decompresses beyond 1 GB; assert `DecompressionTooLarge` returned

### DowngradeBlocked

```rust
/// Cannot downgrade to shared mode while uncommitted changes exist.
#[error("downgrade blocked: uncommitted changes present")]
DowngradeBlocked,
```

**Producers**: `memvid::mod::downgrade_to_shared`
**Consumers**: Callers managing file access mode transitions
**Test**: Add content without committing; call `downgrade_to_shared`; assert `DowngradeBlocked` returned

## Modified Signatures

### SIMD Distance Functions

```rust
// Before (H2 - release mode unsafe)
pub fn l2_distance_squared_simd(a: &[f32], b: &[f32]) -> f32

// After
pub fn l2_distance_squared_simd(a: &[f32], b: &[f32]) -> Result<f32, MemvidError>
```

### CanonicalEncoding::from_byte

```rust
// Before (M4 - returns Default on unknown)
pub fn from_byte(b: u8) -> Self

// After
pub fn from_byte(b: u8) -> Result<Self, MemvidError>
```

### downgrade_to_shared

```rust
// Before (H9 - silent no-op)
pub fn downgrade_to_shared(&mut self)

// After
pub fn downgrade_to_shared(&mut self) -> Result<(), MemvidError>
```
