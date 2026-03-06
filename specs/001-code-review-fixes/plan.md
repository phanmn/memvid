# Implementation Plan: Code Review Fixes

**Branch**: `001-code-review-fixes` | **Date**: 2026-03-05 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `/specs/001-code-review-fixes/spec.md`

## Summary

Address 3 critical, 13 high, and 25 medium priority findings from the memvid code review. The fixes span data integrity (stable hashing, complete rollback), security (XLSX size limits, API key zeroization), release-mode correctness (SIMD validation, HNSW bounds), observability (structured logging, error surfacing), and code quality (dead code removal, KV cache fix). Execution follows the recommended order: C1 → H6 → H7 → C3 → H2 → H8 → C2 → H1 → H4 → H5 → remaining items.

## Technical Context

**Language/Version**: Rust 1.90.0 (Edition 2024, MSRV 1.85.0)
**Primary Dependencies**: blake3 1.5.1, tantivy 0.25, hnsw 0.11, ort 2.0.0-rc.10, tracing 0.1.41, zip 7.1, candle 0.9
**Storage**: Single `.mv2` file (custom binary format with WAL, segments, indices)
**Testing**: `cargo test` (13 integration test files in `tests/`)
**Target Platform**: macOS (aarch64, x86_64), Linux (x86_64), Windows (x86_64)
**Project Type**: Single Rust library crate
**Performance Goals**: Whisper decoding linear in token count (SC-007: 2x improvement for 60s clips)
**Constraints**: No sidecar files, crash-safe WAL, append-only frames, synchronous API
**Scale/Scope**: ~135 source files, ~66,660 LOC; 32 functional requirements across 5 user stories

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

Constitution is an unfilled template — no project-specific gates defined. No violations to check. Gate passes by default.

**Post-Phase 1 re-check**: No new violations introduced. All changes modify existing files within the established `src/` structure. One new dependency (`secrecy`) is lightweight and justified (R7).

## Project Structure

### Documentation (this feature)

```text
specs/001-code-review-fixes/
├── spec.md              # Feature specification (clarified)
├── plan.md              # This file
├── research.md          # Phase 0: research decisions
├── data-model.md        # Phase 1: entity changes and new types
├── quickstart.md        # Phase 1: execution guide
├── contracts/           # Phase 1: API change contracts
│   └── error-variants.md
├── checklists/
│   └── requirements.md  # Pre-existing checklist
└── tasks.md             # Phase 2 output (created by /speckit.tasks)
```

### Source Code (repository root)

```text
src/
├── types/
│   ├── logic_mesh.rs        # C1: BLAKE3 hash + migration
│   ├── search.rs            # H12/H13: Default impl
│   └── common.rs            # M4: CanonicalEncoding error
├── memvid/
│   ├── mutation.rs          # C2/C3: MemvidSnapshot, rollback, tests
│   ├── segments.rs          # M22: dead code removal
│   ├── memory.rs            # M23: unused field removal
│   ├── doctor.rs            # M21: tracing instead of println
│   ├── search/
│   │   └── api.rs           # H8: index error logging
│   └── lifecycle.rs         # H1: defer instant indexing
├── simd.rs                  # H2: runtime dimension check
├── vec.rs                   # M11/M15: bounds check, error handling
├── clip.rs                  # H3: dimension validation
├── whisper.rs               # H10: KV cache fix
├── api_embed.rs             # M8/M9: SecretString, HTTPS warn
├── reader/
│   ├── xlsx.rs              # H7: size limits
│   └── xlsx_ooxml.rs        # M25: midnight boundary
├── structure/
│   └── chunker.rs           # H6: remove String::leak
├── table/
│   └── pdf_extractor.rs     # M17: lazy regex
├── io/
│   ├── time_index.rs        # H5: checksum verification
│   └── temporal_index.rs    # H4: u16 flag enforcement
├── extract.rs               # M20: log → tracing
├── error.rs                 # New error variants
└── text_embed.rs            # M6: BLAKE3 for cache keys

tests/
├── mutation_pipeline.rs     # C2: expanded coverage
├── mutation.rs              # C3: rollback tests
├── lifecycle.rs             # C1: migration tests
├── xlsx_structured.rs       # H7: size limit tests
└── search_orchestration.rs  # H8: error surfacing tests
```

**Structure Decision**: Existing single-crate Rust library structure. All changes are modifications to existing files plus new error variants and one new struct (`MemvidSnapshot`). No structural changes needed.

## Complexity Tracking

No constitution violations to justify. All changes are targeted fixes within the existing architecture.

## Implementation Phases

### Phase 1: Data Integrity (Critical) — C1, C3, C2

| Task | Files | FR | SC |
|------|-------|----|----|
| Replace DefaultHasher with BLAKE3 | `types/logic_mesh.rs` | FR-001 | SC-001 |
| Add auto-migration on open | `types/logic_mesh.rs`, `memvid/lifecycle.rs` | FR-002 | SC-001 |
| Create MemvidSnapshot struct | `memvid/mutation.rs` (or new file) | FR-003 | SC-002 |
| Fix with_staging_lock rollback | `memvid/mutation.rs` | FR-003 | SC-002 |
| Add mutation pipeline tests | `tests/mutation_pipeline.rs` | FR-004 | SC-003 |
| Defer instant indexing to commit | `memvid/mutation.rs` | FR-005 | — |

### Phase 2: Security & Safety — H7, H2, M8, M9

| Task | Files | FR | SC |
|------|-------|----|----|
| XLSX size limits | `reader/xlsx.rs`, `error.rs` | FR-011 | SC-005 |
| SIMD dimension validation | `simd.rs` | FR-006 | SC-004 |
| API key zeroization | `api_embed.rs`, `Cargo.toml` | FR-026 | — |
| HTTPS URL warning | `api_embed.rs` | FR-027 | — |

### Phase 3: Correctness — H1, H3, H4, H5, M11, M15, M4

| Task | Files | FR | SC |
|------|-------|----|----|
| Defer instant indexing to commit | `memvid/mutation.rs` | FR-005 | — |
| CLIP dimension validation | `clip.rs` | FR-007 | SC-004 |
| Enforce u16 flags at API boundary | `io/temporal_index.rs` | FR-008 | — |
| BLAKE3 checksum verification on read | `io/time_index.rs` | FR-009 | — |
| HNSW bounds checking | `vec.rs` | FR-019 | SC-004 |
| Replace catch_unwind in vec decode | `memvid/search/builders.rs` | FR-021 | — |
| CanonicalEncoding error for unknown bytes | `types/common.rs` | FR-018 | — |

### Phase 4: Observability — H8, H9, M20, M21

| Task | Files | FR | SC |
|------|-------|----|----|
| Log index load errors + surface in stats | `memvid/search/api.rs` | FR-012 | SC-006 |
| downgrade_to_shared returns DowngradeBlocked | `memvid/mod.rs` | FR-013 | — |
| Replace log → tracing (3 files) | `extract.rs`, `mutation.rs`, `search/tantivy.rs` | FR-020 | SC-009 |
| Replace println → tracing in doctor.rs | `memvid/doctor.rs` | FR-020 | — |

### Phase 5: Code Quality — H10, M17, M22-M25, remaining

| Task | Files | FR | SC |
|------|-------|----|----|
| Fix Whisper KV cache | `whisper.rs` | FR-014 | SC-007 |
| Lazy regex compilation | `table/pdf_extractor.rs` | FR-022 | — |
| Remove dead code | `memvid/segments.rs`, `memvid/memory.rs` | FR-025 | SC-010 |
| WAL checked arithmetic | `io/wal.rs` | FR-016 | — |
| Model name in cache keys | `text_embed.rs` | FR-017 | — |
| SearchRequest Default impl | `types/search.rs` | FR-015 | — |
| UTF-8 BOM handling | `table/mod.rs` | FR-023 | — |
| Read-only snapshot permissions | `memvid/lifecycle.rs` | FR-024 | — |
| Clamp UTC offset to i16 | `io/temporal_index.rs` | FR-029 | — |
| Clamp byte offsets to u32 | various | FR-030 | — |
| Excel midnight boundary | `reader/xlsx_ooxml.rs` | FR-031 | — |
| Thread-safe stderr suppression | `clip.rs` | FR-028 | — |
| Remove/use ignored regex params | `table/pdf_extractor.rs` | FR-032 | — |

## Generated Artifacts

- [research.md](research.md) — Phase 0: all NEEDS CLARIFICATION resolved
- [data-model.md](data-model.md) — Phase 1: entity changes, new types, error variants
- [quickstart.md](quickstart.md) — Phase 1: execution guide with verification commands
- [contracts/error-variants.md](contracts/error-variants.md) — Phase 1: new error type contracts
