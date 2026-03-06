# AGENTS.md
Practical guidance for coding agents working in this repository.

## Rule Sources
- Primary guidance: `CLAUDE.md`.
- Contributor guidance: `CONTRIBUTING.md`.
- Build/test automation: `Makefile`, `.github/workflows/ci.yml`.
- Cursor rules check:
  - `.cursor/rules/` not present.
  - `.cursorrules` not present.
- Copilot rules check:
  - `.github/copilot-instructions.md` not present.
- If Cursor/Copilot rules are added later, incorporate them here.

## Repository Snapshot
- Crate: `memvid-core`.
- Language: Rust.
- Edition: `2024`.
- Minimum Rust (`Cargo.toml`): `1.85.0`.
- Pinned toolchain (`rust-toolchain.toml`): `1.90.0`.
- Core model: single-file `.mv2` memory engine.
- Key constraints: append-only frames, WAL crash safety, synchronous API.

## Build Commands
Run from repository root:
```bash
cargo build
cargo build --release
cargo check --features "lex,pdf_extract"
cargo build --features "lex,pdf_extract"
cargo build --release --all-features
```
Make shortcuts:
```bash
make check
make build
make build-release
make build-all-features
```

## Format and Lint Commands
```bash
cargo fmt --all
cargo fmt --all -- --check
cargo clippy --all-targets --features "lex,pdf_extract" -- -D warnings
cargo clippy -- -D warnings -A clippy::non_std_lazy_statics
```
Make shortcuts:
```bash
make fmt
make fmt-check
make clippy
make lint
make verify
```

## Test Commands (Single-Test Emphasis)
```bash
cargo test
cargo test --features "lex,pdf_extract"
cargo test -- --nocapture
cargo test search_pagination_and_params
cargo test --lib search_pagination_and_params
cargo test --test lifecycle
cargo test --test lifecycle create_and_open
cargo test --doc --features "lex,pdf_extract"
cargo test --all-targets --features "lex,pdf_extract"
cargo test --features "lex,pdf_extract" -- --no-fail-fast
```
Make shortcuts:
```bash
make test
make test-verbose
make test-unit
make test-integration
make test-doc
make test-all-targets
```
CI note: Windows job uses `-- --test-threads=1`.

## Feature Flags and Gating
- Default features: `lex`, `pdf_extract`, `simd`.
- Heavy optional features: `vec`, `clip`, `whisper`, `encryption`, `logic_mesh`, `replay`.
- Gate feature-specific logic with `#[cfg(feature = "...")]`.
- Keep serialization compatibility for manifests/types when feature-gating behavior.

## Code Style Guidelines
### Formatting and File Hygiene
- Follow `.editorconfig`:
  - UTF-8, LF endings, final newline.
  - 4-space indent for Rust.
  - no trailing whitespace (Markdown exempt).
- Always run `cargo fmt --all` after edits.

### Imports
- Prefer import grouping order:
  1. `std` imports.
  2. external crate imports.
  3. `crate::...` / local module imports.
- Prefer explicit imports over wildcard imports (except clear prelude cases).
- Remove unused imports.

### Naming
- Types/traits/enums: `PascalCase`.
- Functions/modules/files: `snake_case`.
- Constants/statics: `UPPER_SNAKE_CASE`.
- Test names should describe expected behavior.

### Types and API Design
- Prefer explicit types for public APIs.
- Preserve public API stability unless breaking change is intentional.
- Reuse existing builder patterns (e.g., `PutOptions::builder()`).
- Keep feature-gated exports aligned with existing `lib.rs` patterns.

### Error Handling
- Use crate alias: `Result<T> = std::result::Result<T, MemvidError>`.
- Use `thiserror` variants in `MemvidError` for domain errors.
- Prefer `?` and contextual `map_err(...)`.
- In non-test library code, avoid `unwrap()` and `expect()`.
- In tests, `unwrap/expect` is acceptable for setup/assertion clarity.

### Lints and Safety
- Crate policy includes:
  - `#![deny(clippy::all, clippy::pedantic)]`
  - `#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]`
- CI expects zero warnings.
- Do not add broad `#![allow(...)]` entries without clear justification.

### Logging and Diagnostics
- Use `tracing` macros (`debug!`, `info!`, `warn!`, `error!`).
- Keep logs structured and useful; avoid noisy hot-path logging.

### Documentation and Comments
- Add `///` docs for public APIs and important public types.
- Keep comments concise and accurate.
- Prefer clear code over excessive commentary.

### Testing Conventions
- Unit tests: colocated in `#[cfg(test)]` modules.
- Integration tests: `tests/` directory.
- Use `tempfile`/temp dirs for filesystem isolation.
- Add regression tests for bug fixes when practical.

## Architecture Guardrails
- Do not introduce required sidecar files for core `.mv2` behavior.
- Preserve append-only semantics for frames.
- Preserve WAL-first durability behavior.
- Preserve deterministic on-disk format expectations.
- Avoid introducing async runtime dependencies in core crate logic.

## Agent Checklist
- Run `cargo fmt --all`.
- Run strict clippy for touched paths.
- Run targeted single tests first, then broader suite.
- Test feature-gated code with feature on/off when possible.
- Update docs/tests alongside behavior changes.
