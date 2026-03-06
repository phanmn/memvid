# Memvid Codebase Review — Fix Plan

Full codebase review performed 2026-03-05 across 135 source files (~82K lines).
Organized by priority. Each fix includes the problem, location, root cause, and concrete fix.

---

## CRITICAL

### C1. `compute_node_id` uses `DefaultHasher` — on-disk IDs unstable across Rust versions

**File:** `src/types/logic_mesh.rs:566-571`

**Problem:** `std::collections::hash_map::DefaultHasher` is explicitly documented as not
guaranteed to produce the same output across Rust compiler versions. `compute_node_id`
generates deterministic node IDs that are serialized to `.mv2` files and used for adjacency
lookups, edge references, and mesh traversal. Upgrading the Rust toolchain could silently
break deserialization of all existing logic mesh data — nodes referenced by old IDs would
no longer match freshly computed IDs.

**Current code:**
```rust
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

pub fn compute_node_id(kind: &str, label: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    kind.hash(&mut hasher);
    label.hash(&mut hasher);
    hasher.finish()
}
```

**Fix:** Replace `DefaultHasher` with a stable, portable hasher. Two options:

**Option A — BLAKE3 (already a dependency):**
```rust
pub fn compute_node_id(kind: &str, label: &str) -> u64 {
    let mut h = blake3::Hasher::new();
    h.update(kind.as_bytes());
    h.update(b"\x00"); // separator to avoid "ab"+"c" == "a"+"bc"
    h.update(label.as_bytes());
    let hash = h.finalize();
    u64::from_le_bytes(hash.as_bytes()[..8].try_into().unwrap())
}
```

**Option B — FNV or SipHash with fixed keys:**
```rust
use std::hash::{Hash, Hasher};
use siphasher::SipHasher13; // add `siphasher = "1"` to Cargo.toml

pub fn compute_node_id(kind: &str, label: &str) -> u64 {
    let mut hasher = SipHasher13::new_with_keys(0x4d454d56_49440001, 0);
    kind.hash(&mut hasher);
    label.hash(&mut hasher);
    hasher.finish()
}
```

**Migration:** Existing `.mv2` files with logic mesh data need a one-time migration.
Add a version field to `LogicMesh` serialization. On load, if version < 2, rebuild
all node IDs and update edges. Add a `doctor` action for this migration.

**Testing:**
- Unit test: `compute_node_id("entity", "foo")` returns a fixed constant (pin the value)
- Round-trip test: serialize mesh, upgrade hasher, deserialize + migrate, verify traversal

---

### C2. `mutation.rs` is 4,166 lines with zero test coverage

**File:** `src/memvid/mutation.rs`

**Problem:** The entire write path (put, commit, delete, vacuum, WAL staging, chunking,
temporal mention extraction, index rebuild, segment publishing) lives in a single 4,166-line
file with no `#[cfg(test)]` module. The file's own doc comment acknowledges this:
"The long-term structure will split into ingestion/chunking/WAL staging modules."

**Fix — Phase 1: Add tests without splitting (immediate):**

Create `src/memvid/mutation_tests.rs` with `#[cfg(test)]` and add `#[path = "mutation_tests.rs"] mod mutation_tests;` at the bottom of `mutation.rs`. Target these critical paths first:

1. **`put_internal` chunking decisions** — test that content > chunk threshold gets split,
   content <= threshold stays as single frame
2. **`apply_records` frame ID assignment** — verify frame IDs match `toc.frames.len()`,
   not WAL sequence numbers
3. **`with_staging_lock` rollback** — create a scenario where staging fails mid-operation,
   verify all fields are restored (see C3)
4. **Capacity enforcement** — test put fails when capacity exceeded
5. **Delete + vacuum** — test frame deletion marks tombstone, vacuum reclaims space
6. **Temporal mention extraction** — test `extract_temporal_mentions` with known inputs

**Fix — Phase 2: Split the file (follow-up):**

Proposed module structure:
```
src/memvid/
  mutation/
    mod.rs           — pub API: put_bytes, put_bytes_with_options, commit, delete
    staging.rs       — with_staging_lock, save/restore logic
    chunking.rs      — content splitting, chunk boundary detection
    wal_apply.rs     — apply_records, WAL replay
    indexing.rs      — rebuild_indexes, instant indexing
    temporal.rs      — extract_temporal_mentions, temporal mention storage
    vacuum.rs        — vacuum, compaction
    segments.rs      — publish_lex_delta, publish_vec_delta, publish_time_delta
```

Each module should be 400-800 lines with its own test module.

---

### C3. Incomplete rollback in `with_staging_lock` corrupts state on failure

**File:** `src/memvid/mutation.rs:416-497`

**Problem:** The staging lock saves and restores certain fields on failure, but omits:
- `memories_track`
- `logic_mesh`
- `sketch_track`
- `clip_enabled` / `clip_index`
- `vec_enabled` / `vec_index` / `vec_model` / `vec_compression`
- Feature-gated fields (`temporal_track`, etc.)

A failed staging operation that partially modified these fields leaves them inconsistent
with the committed state.

**Current save/restore pattern (simplified):**
```rust
// Save
let saved_toc = self.toc.clone();
let saved_wal = self.wal.clone();
let saved_dirty = self.dirty;
// ... some fields saved, many missing

// Execute operation
let result = operation(self);

// On failure, restore
if result.is_err() {
    self.toc = saved_toc;
    self.wal = saved_wal;
    self.dirty = saved_dirty;
    // Missing fields NOT restored!
}
```

**Fix — Option A: Save all mutable state in a snapshot struct:**
```rust
struct MemvidSnapshot {
    toc: Toc,
    wal: Wal,
    dirty: bool,
    memories_track: Option<MemoriesTrack>,
    logic_mesh: Option<LogicMesh>,
    sketch_track: Option<SketchTrack>,
    vec_enabled: bool,
    vec_index: Option<HnswVecIndex>,
    vec_model: Option<String>,
    vec_compression: VectorCompression,
    #[cfg(feature = "clip")]
    clip_enabled: bool,
    #[cfg(feature = "clip")]
    clip_index: Option<ClipIndex>,
    // ... all mutable fields
}

impl Memvid {
    fn snapshot(&self) -> MemvidSnapshot { /* clone all fields */ }
    fn restore(&mut self, snap: MemvidSnapshot) { /* restore all fields */ }
}
```

**Fix — Option B: Use a transaction log pattern:**
Instead of save/restore, record mutations as a log of operations. On failure, replay
the inverse. This is more complex but avoids the "forgot to add a field" problem.

**Fix — Option C (simplest): Clone the entire Memvid on staging entry:**
```rust
fn with_staging_lock<F, T>(&mut self, f: F) -> Result<T>
where F: FnOnce(&mut Self) -> Result<T> {
    let backup = self.clone(); // requires Memvid: Clone
    match f(self) {
        Ok(v) => Ok(v),
        Err(e) => {
            *self = backup;
            Err(e)
        }
    }
}
```
This requires `Memvid` to implement `Clone`. If the struct is too large to clone
efficiently, use Option A with a compile-time assertion that the snapshot struct has
the same number of fields as `Memvid` (via a `const FIELD_COUNT` approach or a proc macro).

**Testing:**
- Add a test that forces a failure mid-staging (e.g., by hitting capacity limit after
  partial work), then verifies ALL fields match their pre-staging values
- Add a static assertion or doc comment listing all fields that must be saved

---

## HIGH

### H1. Wrong frame IDs during instant indexing

**File:** `src/memvid/mutation.rs:3837, 3899, 3937`

**Problem:** During `put_internal`, instant indexing uses `parent_seq` (WAL sequence number)
as the frame ID when adding documents to the Tantivy index. But actual frame IDs are
assigned during `apply_records` as `self.toc.frames.len()`. Any search between `put` and
`commit` returns hits referencing WAL sequence numbers instead of frame IDs.

The `rebuild_indexes` function (line ~2131) already acknowledges this:
"frames were added to Tantivy with WAL sequence numbers as IDs, which don't match
the actual frame IDs."

**Fix:**
Two approaches:

**Option A — Don't instant-index; defer to commit:**
Remove instant indexing entirely. Accept that search results are only available after
`commit()`. This is simpler and eliminates the mismatch.

**Option B — Use a temporary ID mapping:**
```rust
// During put_internal, record the mapping
self.pending_seq_to_frame.insert(parent_seq, None); // frame ID not yet known

// During apply_records, fill in the mapping
let frame_id = self.toc.frames.len() as FrameId;
if let Some(entry) = self.pending_seq_to_frame.get_mut(&seq) {
    *entry = Some(frame_id);
}

// During search (before commit), translate IDs
fn translate_pending_id(&self, id: FrameId) -> Option<FrameId> {
    self.pending_seq_to_frame.values()
        .find(|&&v| v == Some(id))
        .copied()
        .flatten()
}
```

**Testing:**
- Put content, search BEFORE commit, verify returned frame IDs are valid
- Put content, commit, search again, verify same content is found with correct IDs

---

### H2. SIMD distance functions skip length check in release builds

**File:** `src/simd.rs:17`

**Problem:** `debug_assert_eq!(a.len(), b.len())` is stripped in release. If vectors of
different lengths are compared, the function either computes garbage distances (processing
only `min(a.len(), b.len())` elements) or panics on out-of-bounds access in the SIMD
chunk path where `b[offset + 7]` can overflow.

**Fix:**
```rust
// Replace debug_assert_eq! with a real check
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    assert_eq!(a.len(), b.len(), "vector dimension mismatch: {} vs {}", a.len(), b.len());
    // ... rest of function
}
```

Or, if the assertion cost matters in hot paths, validate at the call site (in
`HnswVecIndex::search`) and use `unsafe` with a safety comment in the SIMD function:

```rust
/// # Safety
/// Caller must ensure `a.len() == b.len()`.
pub unsafe fn cosine_similarity_unchecked(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len());
    // ...
}
```

**Testing:**
- Test that mismatched dimensions produce an error (not garbage or panic)

---

### H3. CLIP embedding dimension not validated after inference

**File:** `src/clip.rs:959, 1111`

**Problem:** Both `encode_image` and `encode_text` extract the output tensor as
`data.to_vec()` without verifying the result matches `self.model_info.dims`. If the ONNX
model returns a different shape (e.g., `[1, seq_len, hidden_dim]` flattened), the entire
tensor becomes the embedding — a wrong-dimension vector enters the index silently.

**Fix:**
```rust
let embedding = data.to_vec();
let expected_dim = self.model_info.dims as usize;
if embedding.len() != expected_dim {
    return Err(MemvidError::Other(format!(
        "CLIP model returned {} dimensions, expected {}",
        embedding.len(), expected_dim
    )));
}
```

Compare with `text_embed.rs:778` which correctly does `.take(embedding_dim)`.

**Testing:**
- Mock an ONNX session that returns wrong dimensions, verify error

---

### H4. Temporal track flags silently truncated u32 to u16

**File:** `src/io/temporal_index.rs:213, 358`

**Problem:** `append_track` takes `flags: u32` but stores it as `flags as u16`, silently
discarding the upper 16 bits. `read_track` reads `u16` and casts back to `u32`.

**Fix — Option A: Widen the on-disk format to u32:**
```rust
// In append_track:
buf.extend_from_slice(&flags.to_le_bytes()); // 4 bytes instead of 2

// In read_track:
let flags = u32::from_le_bytes([data[off], data[off+1], data[off+2], data[off+3]]);
```
This changes the binary format — requires a version bump in the temporal track header.

**Fix — Option B: Enforce u16 range at the API level:**
```rust
pub fn append_track(&mut self, ..., flags: u16) -> Result<()> {
```
Change the `TemporalTrack.flags` and `TemporalTrackManifest.flags` fields to `u16`.

**Testing:**
- Round-trip test: write flags with bits > 0xFFFF, read back, verify they survive

---

### H5. Time index `read_track` never verifies checksum

**File:** `src/io/time_index.rs:61-129`

**Problem:** `read_track` validates magic, count, payload length, and sort order, but never
recomputes or verifies the BLAKE3 checksum. Compare with `temporal_index::read_track`
which does verify checksums.

**Fix:**
```rust
// After reading data, before returning:
let computed = calculate_checksum(&entries_bytes);
if computed != manifest.checksum {
    return Err(MemvidError::ChecksumMismatch {
        expected: hex::encode(&manifest.checksum),
        actual: hex::encode(&computed),
    });
}
```

**Testing:**
- Corrupt a single byte in a time index, verify `read_track` returns checksum error

---

### H6. `String::leak()` memory leak in structural chunker

**File:** `src/structure/chunker.rs:112`

**Problem:** `heading.format().leak()` converts a `String` to `&'static str`, permanently
leaking memory. Called per heading element during document chunking.

**Current code:**
```rust
pending_heading = Some(heading.format().leak());
```

**Fix:** Change `pending_heading` from `Option<&'static str>` to `Option<String>`:
```rust
// In the struct or local variable:
let mut pending_heading: Option<String> = None;

// At the assignment:
pending_heading = Some(heading.format());

// Where it's consumed, use .as_deref() or .as_str():
if let Some(ref heading) = pending_heading {
    chunk.heading = Some(heading.clone());
}
```

If `pending_heading` feeds into a struct that requires `&str` lifetime, use `Arc<str>`:
```rust
pending_heading = Some(Arc::from(heading.format()));
```

**Testing:**
- Process a document with 10,000 headings, verify memory usage is bounded
- Or simply grep for `.leak()` in the codebase and eliminate all non-test uses

---

### H7. XLSX has no size limits — zip bomb / OOM potential

**File:** `src/reader/xlsx.rs` (entire), `src/reader/xlsx_ooxml.rs:227-261, 270`

**Problem:** Unlike PDF reader which has `PDFIUM_MAX_BYTES` (512 MB) and
`PDF_LOPDF_MAX_BYTES` (32 MB), XLSX readers have no validation before decompressing.
`read_zip_entry` reads an entire zip entry into a `String` with no size limit.

**Fix:**
```rust
// Add constants at the top of xlsx.rs:
const XLSX_MAX_FILE_BYTES: u64 = 512 * 1024 * 1024; // 512 MB compressed
const XLSX_MAX_ENTRY_BYTES: u64 = 64 * 1024 * 1024;  // 64 MB per entry

// In the reader, before processing:
if data.len() as u64 > XLSX_MAX_FILE_BYTES {
    return Err(MemvidError::Other(format!(
        "XLSX file too large: {} bytes (max {})",
        data.len(), XLSX_MAX_FILE_BYTES
    )));
}

// In read_zip_entry, use a bounded read:
fn read_zip_entry(file: &mut ZipFile, max_bytes: u64) -> Result<String> {
    let size = file.size();
    if size > max_bytes {
        return Err(MemvidError::Other(format!(
            "XLSX entry too large: {} bytes (max {})",
            size, max_bytes
        )));
    }
    let mut buf = String::with_capacity(size as usize);
    file.read_to_string(&mut buf)?;
    Ok(buf)
}
```

**Testing:**
- Create a small XLSX with a zip entry that decompresses to > 64 MB, verify rejection

---

### H8. Silent swallowing of index load errors

**File:** `src/memvid/search/api.rs:100-106, 136-144`

**Problem:** When loading lex or vec index from manifest, read/decode errors are caught
and the index is set to `None` with `Ok(())` returned. The caller gets empty search
results with no diagnostic.

**Fix:** Log the error at `warn!` level and surface it in stats:
```rust
pub fn load_lex_index_from_manifest(&mut self) -> Result<()> {
    match self.try_load_lex_index() {
        Ok(index) => { self.lex_index = Some(index); Ok(()) }
        Err(e) => {
            tracing::warn!("Failed to load lex index: {e}. Search will return no results.");
            self.lex_load_error = Some(e.to_string());
            Ok(())
        }
    }
}
```

Add `lex_load_error: Option<String>` and `vec_load_error: Option<String>` fields to
`Memvid`. Surface these in `stats()` output so callers can detect degraded state.

**Testing:**
- Corrupt a lex index segment, open the file, verify warning is logged and stats report it

---

### H9. `downgrade_to_shared` silently succeeds when dirty

**File:** `src/lib.rs:428-438`

**Problem:** When the instance has uncommitted changes (`self.dirty` or
`tantivy_index_pending()`), returns `Ok(())` without downgrading. Caller cannot detect.

**Fix:**
```rust
pub fn downgrade_to_shared(&mut self) -> Result<bool> {
    if self.dirty || self.tantivy_index_pending() {
        tracing::warn!("Cannot downgrade: uncommitted changes present");
        return Ok(false); // Return false to indicate no-op
    }
    // ... actual downgrade logic
    Ok(true)
}
```

Or return `Err(MemvidError::DirtyState)` if the caller should treat this as an error.

---

### H10. Whisper decoder is O(n^2)

**File:** `src/whisper.rs:940-987`

**Problem:** `flush_kv_cache` is always `true` (lines 964-966), defeating the KV cache.
Every decoding step re-processes all previous tokens.

**Fix:**
```rust
// Only flush on first token:
let flush_kv_cache = token_idx == 0;
```

Or if the candle KV cache behavior requires always flushing, add incremental token
passing:
```rust
// Pass only the new token after first step:
let input_tokens = if token_idx == 0 {
    &all_tokens[..]
} else {
    &all_tokens[all_tokens.len() - 1..]
};
```

**Testing:**
- Benchmark transcription of a 60-second audio clip before/after fix
- Verify output is identical (no regression from KV cache change)

---

### H11. `doctor.rs` (1,686 lines) has zero test coverage

**File:** `src/memvid/doctor.rs`

**Problem:** Performs destructive file repairs (rebuild indexes, fix headers, repair WAL)
with only a single integration test in `lib.rs`.

**Fix:** Add test module covering:
1. `diagnose` — given a healthy file, returns no issues
2. `diagnose` — given a corrupted header, detects issue
3. `repair` — given a missing lex index, rebuilds it
4. `repair` — given a truncated WAL, recovers committed entries
5. `repair` — verify repair is idempotent (running twice produces same result)

Create test fixtures with known corruptions rather than relying on random corruption.

---

### H12. Feature-flag conditional struct fields break downstream compilation

**File:** `src/types/search.rs:41` (SearchRequest), also TimelineQuery

**Problem:** `#[cfg(feature = "temporal_track")] temporal: Option<TemporalFilter>` means
struct literal construction breaks when the feature flag changes.

**Fix:**
```rust
// Always include the field, gate the type:
#[cfg(feature = "temporal_track")]
pub temporal: Option<TemporalFilter>,
#[cfg(not(feature = "temporal_track"))]
pub temporal: Option<()>, // always None, zero-cost
```

Or better, implement `Default`:
```rust
impl Default for SearchRequest {
    fn default() -> Self {
        Self {
            query: String::new(),
            top_k: 10,
            snippet_chars: 200,
            // ... all other fields with sensible defaults
        }
    }
}
```

Then callers use:
```rust
let request = SearchRequest {
    query: "hello".into(),
    top_k: 5,
    ..Default::default()
};
```

---

### H13. `SearchRequest` has no `Default` or builder

**File:** `src/types/search.rs:41`

**Problem:** 11+ required fields force verbose construction at every call site.

**Fix:** Implement `Default` (see H12 above) and optionally a builder:
```rust
impl SearchRequest {
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            ..Default::default()
        }
    }

    pub fn top_k(mut self, k: usize) -> Self { self.top_k = k; self }
    pub fn snippet_chars(mut self, n: usize) -> Self { self.snippet_chars = n; self }
    pub fn scope(mut self, s: impl Into<String>) -> Self { self.scope = Some(s.into()); self }
    // ... etc
}
```

---

## MEDIUM

### M1. `MemoriesTrack::get_card` is O(n)

**File:** `src/types/memories_track.rs:298`

**Fix:** Since card IDs are sequential from `next_id`, use direct index lookup:
```rust
pub fn get_card(&self, id: u32) -> Option<&MemoryCard> {
    self.cards.get(id as usize).filter(|c| c.id == id)
}
```

---

### M2. `LogicMesh.nodes` and `edges` are `pub` but adjacency depends on `rebuild()`

**File:** `src/types/logic_mesh.rs:274-276`

**Fix:** Change to `pub(crate)` and add mutation methods that maintain the invariant:
```rust
pub(crate) nodes: Vec<MeshNode>,
pub(crate) edges: Vec<MeshEdge>,

pub fn add_node(&mut self, node: MeshNode) {
    self.nodes.push(node);
    self.rebuild_adjacency();
}
```

---

### M3. `EnrichmentQueueManifest.tasks` unbounded deserialization

**File:** `src/types/manifest.rs:867`

**Fix:** Add `#[serde(deserialize_with = "deserialize_vec_bounded")]` with a reasonable
limit (e.g., 100,000 tasks), matching the pattern used for `Toc.frames` and `Toc.segments`.

---

### M4. `CanonicalEncoding::from_byte` silently defaults to `Plain`

**File:** `src/types/common.rs:22-27`

**Fix:**
```rust
pub fn from_byte(b: u8) -> Result<Self> {
    match b {
        0 => Ok(Self::Plain),
        1 => Ok(Self::Zstd),
        _ => Err(MemvidError::UnsupportedEncoding(b)),
    }
}
```

---

### M5. WAL sequence `+` vs `wrapping_add` inconsistency

**File:** `src/io/wal.rs:138, 149`

**Fix:** Use `wrapping_add` consistently on both lines (or `checked_add` with error).

---

### M6. Embedding cache key lacks model name

**File:** `src/text_embed.rs:603-607`

**Fix:** Include model name in cache key:
```rust
let mut hasher = DefaultHasher::new();
self.config.model_name.hash(&mut hasher);
text.hash(&mut hasher);
let key = hasher.finish();
```

---

### M7. PQ hardcoded to 384-dim only

**File:** `src/vec_pq.rs:28-31`

**Fix:** Make `NUM_SUBSPACES` and `SUBSPACE_DIM` configurable or compute from actual
embedding dimension. At minimum, document this limitation prominently in the public API
and return a clear error message.

---

### M8. API key not zeroized

**File:** `src/api_embed.rs:235`

**Fix:** Add `secrecy = "0.8"` to Cargo.toml, use `SecretString`:
```rust
use secrecy::{SecretString, ExposeSecret};

pub struct OpenAIEmbedder {
    api_key: SecretString,
    // ...
}
```

---

### M9. `base_url` not validated — keys could leak over HTTP

**File:** `src/api_embed.rs:290`

**Fix:**
```rust
if !self.config.base_url.starts_with("https://") {
    tracing::warn!("OpenAI base_url does not use HTTPS: {}", self.config.base_url);
}
```

Or reject non-HTTPS entirely:
```rust
if !self.config.base_url.starts_with("https://") && !self.config.base_url.starts_with("http://localhost") {
    return Err(MemvidError::Other("base_url must use HTTPS".into()));
}
```

---

### M10. Stderr suppression via `dup2` is not thread-safe

**File:** `src/text_embed.rs:49-96`, also `src/clip.rs`

**Fix:** Move stderr suppression inside the existing `Mutex` guard that protects the
ONNX session, so only one thread manipulates fd 2 at a time:
```rust
let _session_guard = SESSION_MUTEX.lock().unwrap();
let _stderr_guard = SuppressStderr::new(); // now protected by mutex
let result = session.run(...);
// stderr restored when _stderr_guard drops, still under mutex
```

---

### M11. HNSW search can panic on corrupted index

**File:** `src/vec.rs:428`

**Fix:** Add bounds check:
```rust
let frame_id = self.ids.get(neighbor.index)
    .copied()
    .ok_or_else(|| MemvidError::Other(format!(
        "HNSW index corruption: neighbor index {} out of bounds ({})",
        neighbor.index, self.ids.len()
    )))?;
```

---

### M12. Tantivy STRING field with tokenizer — semantic mismatch

**File:** `src/search/tantivy/schema.rs:25-33`

**Fix:** Use `TEXT` field type instead of `STRING` when applying a tokenizer, or use
`STRING` without a tokenizer for exact-match fields:
```rust
// For tokenized search:
let text_opts = TextOptions::default()
    .set_indexing_options(TextFieldIndexing::default()
        .set_tokenizer("memvid_default"));
builder.add_text_field("tags", text_opts);

// For exact match:
builder.add_text_field("tag_exact", STRING);
```

---

### M13. `whole_minutes() as i16` truncation

**File:** `src/memvid/mutation.rs:1673, 1719, 1729`

**Fix:**
```rust
let minutes = dt.offset().whole_minutes();
let tz_offset = i16::try_from(minutes).unwrap_or_else(|_| {
    tracing::warn!("UTC offset {minutes} exceeds i16 range, clamping");
    minutes.clamp(i16::MIN as i32, i16::MAX as i32) as i16
});
```

---

### M14. Byte offsets silently clamped to u32

**File:** `src/memvid/mutation.rs:1652-1653`

**Fix:** Log a warning when clamping occurs:
```rust
let byte_start_u32 = if byte_start > u32::MAX as usize {
    tracing::warn!("Temporal mention byte offset {byte_start} exceeds u32::MAX, clamping");
    u32::MAX
} else {
    byte_start as u32
};
```

---

### M15. `catch_unwind(AssertUnwindSafe(...))` for vec index

**File:** `src/memvid/search/api.rs:145-153`

**Fix:** Replace with proper error handling. If the decoder can panic, wrap it in a
function that returns `Result` and handle the error path explicitly:
```rust
match self.try_decode_vec_index(&data) {
    Ok(index) => { self.vec_index = Some(index); }
    Err(e) => {
        tracing::warn!("Vec index decode failed: {e}");
        self.vec_load_error = Some(e.to_string());
    }
}
```

---

### M16. `ScopedLogLevel` races on global log level

**File:** `src/extract.rs:534-565`

**Fix:** Remove `ScopedLogLevel` entirely. Use `tracing`'s per-span filtering instead:
```rust
let _span = tracing::info_span!("pdf_extraction", level = "warn").entered();
```

Or use a `tracing_subscriber` layer with dynamic filtering per operation.

---

### M17. Regex compiled on every call in pdf_extractor

**File:** `src/table/pdf_extractor.rs:676-679, 962`

**Fix:** Use `LazyLock` (matching the pattern in `auto_tag.rs` and `temporal.rs`):
```rust
use std::sync::LazyLock;

static NUMBER_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\d+").unwrap());
static CURRENCY_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\$[\d,]+").unwrap());
```

---

### M18. BOM detection inconsistency in table/mod.rs

**File:** `src/table/mod.rs:107-115`

**Fix:** Align with `reader/pdf.rs` — check for the 3-byte BOM prefix:
```rust
fn is_pdf_magic(data: &[u8]) -> bool {
    let data = if data.starts_with(&[0xEF, 0xBB, 0xBF]) {
        &data[3..] // Skip UTF-8 BOM
    } else {
        data
    };
    data.starts_with(b"%PDF")
}
```

---

### M19. `open_read_only_snapshot` opens file with write permission

**File:** `src/memvid/lifecycle.rs:475`

**Fix:**
```rust
let file = OpenOptions::new()
    .read(true)
    .write(false) // truly read-only
    .open(path)?;
```

---

### M20. Mixed `log` and `tracing` crates

**Files:** `src/memvid/mutation.rs:20`, `src/search/tantivy.rs:18`

**Fix:** Replace all `use log::{info, warn, ...}` with `use tracing::{info, warn, ...}`.
The API is identical, so this is a drop-in replacement. Remove `log` from Cargo.toml
dependencies if it becomes unused.

---

### M21. `doctor.rs` uses `println!` instead of structured logging

**File:** `src/memvid/doctor.rs:46, 1481`

**Fix:** Replace `doctor_log!` macro internals:
```rust
macro_rules! doctor_log {
    ($($arg:tt)*) => {
        tracing::info!(target: "memvid::doctor", $($arg)*);
    };
}
```

---

### M22. Dead code in mutation.rs

**File:** `src/memvid/mutation.rs:1825-1996`

**Fix:** Remove `publish_lex_delta`, `publish_vec_delta`, `publish_time_delta`, and
`publish_temporal_delta` (all marked `#[allow(dead_code)]`). If needed in the future,
they can be recovered from git history.

---

### M23. `_entity_count` field never used

**File:** `src/memvid/memory.rs:35`

**Fix:** Either remove the field or increment it where entities are counted:
```rust
// If intended to be used:
stats._entity_count = stats.entities.len();
// If not needed:
// Remove the field entirely
```

---

### M24. `collect_table_region` ignores `_percent_re` and `_hours_re`

**File:** `src/table/pdf_extractor.rs:824-826`

**Fix:** Either use these regexes to filter lines within the region, or remove the
parameters from the function signature to avoid confusion.

---

### M25. `excel_serial_to_iso` boundary value at midnight

**File:** `src/reader/xlsx_ooxml.rs:481-485`

**Fix:**
```rust
let total_seconds = (frac * 86400.0).round() as u32;
let total_seconds = total_seconds.min(86399); // clamp to 23:59:59
```

---

## Architecture / Hygiene (Non-blocking)

### A1. `pub use constants::*` leaks internals

**File:** `src/lib.rs:157`

**Fix:** Replace glob with explicit re-exports:
```rust
pub use constants::{MAGIC, SPEC_MAJOR, SPEC_MINOR, HEADER_SIZE};
```

### A2. 25+ `pub mod` declarations expose internals

**File:** `src/lib.rs`

**Fix:** Change internal modules to `pub(crate) mod` and re-export only the public types.

### A3. CLAUDE.md architecture diagram outdated

**File:** `CLAUDE.md`

**Fix:** Update the `src/` tree to include all modules: `analysis/`, `enrich/`, `triplet/`,
`graph_search.rs`, `structure/`, `table/`, `pii.rs`, `simd.rs`, `extract_budgeted.rs`,
`registry.rs`, `lockfile.rs`, `toc.rs`, `replay/`, `encryption/`, `api_embed.rs`, and
the `memvid/search/` subdirectory.

### A4. 22 files exceed 800-line guideline

See full list in the review summary. Priority splits:
1. `mutation.rs` (4,166) — see C2
2. `clip.rs` (1,754) — split into `clip/model.rs`, `clip/encode.rs`, `clip/tests.rs`
3. `doctor.rs` (1,686) — split into `doctor/diagnose.rs`, `doctor/repair.rs`
4. `ask.rs` (1,613) — split RAG query handling from citation building

---

## Execution Order

Recommended fix sequence:

1. **C1** (DefaultHasher) — data corruption risk, fix before more data is written
2. **H6** (String::leak) — memory leak, one-line fix
3. **H7** (XLSX size limits) — security, straightforward to add
4. **C3** (staging rollback) — data integrity
5. **H2** (SIMD bounds check) — correctness, one-line fix
6. **H8** (index error logging) — observability
7. **C2** (mutation.rs tests) — phase 1 only, add tests before splitting
8. **H1** (instant indexing IDs) — correctness
9. **H4** (temporal flags truncation) — data integrity
10. **H5** (time index checksum) — data integrity
11. Everything else in severity order
