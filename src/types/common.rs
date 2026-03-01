//! Foundational enums and marker types shared across memvid data structures.

use std::{marker::PhantomData, path::PathBuf};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Frame IDs are dense u64 indexes into the frame list.
pub type FrameId = u64;

/// Segment IDs identify embedded index segments; monotonic within a file.
pub type SegmentId = u64;

/// Encoding used for the canonical document bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanonicalEncoding {
    Plain,
    Zstd,
}

impl CanonicalEncoding {
    #[must_use]
    pub const fn from_byte(value: u8) -> Self {
        match value {
            0 => CanonicalEncoding::Plain,
            1 => CanonicalEncoding::Zstd,
            _ => CanonicalEncoding::Plain,
        }
    }

    #[must_use]
    pub const fn as_byte(self) -> u8 {
        match self {
            CanonicalEncoding::Plain => 0,
            CanonicalEncoding::Zstd => 1,
        }
    }
}

impl Default for CanonicalEncoding {
    fn default() -> Self {
        Self::Plain
    }
}

impl Serialize for CanonicalEncoding {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u32(u32::from(self.as_byte()))
    }
}

impl<'de> Deserialize<'de> for CanonicalEncoding {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = u32::deserialize(deserializer)?;
        Ok(CanonicalEncoding::from_byte((value & 0xFF) as u8))
    }
}

/// Tier captures the capacity and entitlement envelope for a memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    /// Free tier with small capacity.
    Free,
    /// Developer tier with higher caps.
    Dev,
    /// Enterprise tier with the largest caps.
    Enterprise,
}

impl Tier {
    /// Maximum nominal capacity in bytes for the tier.
    ///
    /// Can be overridden at runtime via the `MEMVID_FREE_CAPACITY_BYTES`,
    /// `MEMVID_DEV_CAPACITY_BYTES`, or `MEMVID_ENTERPRISE_CAPACITY_BYTES`
    /// environment variables respectively.
    #[must_use]
    pub fn capacity_bytes(self) -> u64 {
        match self {
            Tier::Free => env_capacity("MEMVID_FREE_CAPACITY_BYTES", 5 * 1024 * 1024 * 1024), // 5 GB
            Tier::Dev => env_capacity("MEMVID_DEV_CAPACITY_BYTES", 20 * 1024 * 1024 * 1024), // 20 GB
            Tier::Enterprise => {
                env_capacity("MEMVID_ENTERPRISE_CAPACITY_BYTES", 100 * 1024 * 1024 * 1024)
            } // 100 GB
        }
    }
}

/// Read a capacity override from an environment variable, falling back to `default`.
fn env_capacity(var: &str, default: u64) -> u64 {
    std::env::var(var)
        .ok()
        .and_then(|v| parse_capacity_override(&v))
        .unwrap_or(default)
}

fn parse_capacity_override(raw: &str) -> Option<u64> {
    raw.trim().parse::<u64>().ok().filter(|&value| value > 0)
}

/// Marker type signifying an open (mutable) memory.
pub struct Open;

/// Marker type signifying a sealed (read-only) memory.
pub struct Sealed;

/// Mode phantom tracked using [`MemvidHandle<Mode>`].
#[derive(Debug, Clone)]
pub struct MemvidHandle<Mode> {
    pub path: PathBuf,
    pub(crate) _mode: PhantomData<Mode>,
}

/// Marker describing the lifecycle state of a frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrameStatus {
    Active,
    Superseded,
    Deleted,
}

impl Default for FrameStatus {
    fn default() -> Self {
        Self::Active
    }
}

/// Role attributed to a frame in the timeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum FrameRole {
    #[default]
    Document,
    DocumentChunk,
    /// Extracted image from a document (e.g., PDF page image for CLIP)
    ExtractedImage,
}

/// Enrichment state for progressive ingestion.
///
/// Frames start as `Searchable` (instant indexed with skim text) and
/// progress to `Enriched` (full text + embeddings + memory cards).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum EnrichmentState {
    /// Phase 1 complete: searchable via skim text.
    /// Lexical search works, but may have reduced accuracy.
    #[default]
    Searchable = 0,
    /// Phase 2 complete: full text extracted, embeddings generated.
    /// Full search accuracy, semantic search available.
    Enriched = 1,
}

impl EnrichmentState {
    /// Returns true if this frame needs background enrichment.
    #[must_use]
    pub fn needs_enrichment(&self) -> bool {
        matches!(self, Self::Searchable)
    }

    /// Returns true if this frame has full semantic search capability.
    #[must_use]
    pub fn has_embeddings(&self) -> bool {
        matches!(self, Self::Enriched)
    }
}

/// Task in the enrichment queue, representing pending background work.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrichmentTask {
    /// Frame ID to enrich.
    pub frame_id: FrameId,
    /// Timestamp when task was created.
    pub created_at: u64,
    /// Number of chunks already embedded (for resume after crash).
    pub chunks_done: u32,
    /// Total chunks to embed (0 if not yet known).
    pub chunks_total: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_capacity_override_accepts_valid_values() {
        assert_eq!(parse_capacity_override("1"), Some(1));
        assert_eq!(parse_capacity_override("  2048  "), Some(2048));
    }

    #[test]
    fn parse_capacity_override_rejects_invalid_values() {
        assert_eq!(parse_capacity_override(""), None);
        assert_eq!(parse_capacity_override("0"), None);
        assert_eq!(parse_capacity_override("-1"), None);
        assert_eq!(parse_capacity_override("not-a-number"), None);
    }

    #[test]
    fn env_capacity_uses_default_when_var_is_missing() {
        let key = "MEMVID_TEST_MISSING_CAPACITY_BYTES";
        assert_eq!(env_capacity(key, 4096), 4096);
    }

    #[test]
    fn canonical_encoding_from_byte_round_trips() {
        assert_eq!(CanonicalEncoding::from_byte(0), CanonicalEncoding::Plain);
        assert_eq!(CanonicalEncoding::from_byte(1), CanonicalEncoding::Zstd);
        assert_eq!(CanonicalEncoding::Plain.as_byte(), 0);
        assert_eq!(CanonicalEncoding::Zstd.as_byte(), 1);
        // Round-trip
        assert_eq!(
            CanonicalEncoding::from_byte(CanonicalEncoding::Plain.as_byte()),
            CanonicalEncoding::Plain
        );
        assert_eq!(
            CanonicalEncoding::from_byte(CanonicalEncoding::Zstd.as_byte()),
            CanonicalEncoding::Zstd
        );
    }

    #[test]
    fn canonical_encoding_unknown_byte_defaults_to_plain() {
        assert_eq!(CanonicalEncoding::from_byte(2), CanonicalEncoding::Plain);
        assert_eq!(CanonicalEncoding::from_byte(42), CanonicalEncoding::Plain);
        assert_eq!(CanonicalEncoding::from_byte(255), CanonicalEncoding::Plain);
    }

    #[test]
    fn tier_capacity_bytes_are_non_zero_and_ordered() {
        // RAII guard that restores env vars even if assertions panic.
        struct EnvGuard {
            saved: Vec<(&'static str, Option<String>)>,
        }
        impl Drop for EnvGuard {
            fn drop(&mut self) {
                for (var, val) in &self.saved {
                    // SAFETY: single-threaded test; no other thread reads these vars.
                    match val {
                        Some(v) => unsafe { std::env::set_var(var, v) },
                        None => unsafe { std::env::remove_var(var) },
                    }
                }
            }
        }

        let vars: &[&str] = &[
            "MEMVID_FREE_CAPACITY_BYTES",
            "MEMVID_DEV_CAPACITY_BYTES",
            "MEMVID_ENTERPRISE_CAPACITY_BYTES",
        ];
        let _guard = EnvGuard {
            saved: vars
                .iter()
                .map(|&v| (v, std::env::var(v).ok()))
                .collect(),
        };
        for var in vars {
            // SAFETY: single-threaded test context.
            unsafe { std::env::remove_var(var) };
        }

        let free = Tier::Free.capacity_bytes();
        let dev = Tier::Dev.capacity_bytes();
        let enterprise = Tier::Enterprise.capacity_bytes();

        assert!(free > 0, "Free tier capacity must be non-zero");
        assert!(dev > 0, "Dev tier capacity must be non-zero");
        assert!(enterprise > 0, "Enterprise tier capacity must be non-zero");
        assert!(
            enterprise >= dev,
            "Enterprise capacity must be >= Dev capacity"
        );
        assert!(dev >= free, "Dev capacity must be >= Free capacity");
    }

    #[test]
    fn frame_status_variants_are_distinct() {
        let active = FrameStatus::Active;
        let superseded = FrameStatus::Superseded;
        let deleted = FrameStatus::Deleted;
        assert_ne!(active, superseded);
        assert_ne!(active, deleted);
        assert_ne!(superseded, deleted);
    }

    #[test]
    fn frame_status_default_is_active() {
        assert_eq!(FrameStatus::default(), FrameStatus::Active);
    }

    #[test]
    fn enrichment_state_needs_enrichment_logic() {
        let searchable = EnrichmentState::Searchable;
        let enriched = EnrichmentState::Enriched;
        assert!(searchable.needs_enrichment());
        assert!(!enriched.needs_enrichment());
    }

    #[test]
    fn enrichment_state_has_embeddings_logic() {
        let searchable = EnrichmentState::Searchable;
        let enriched = EnrichmentState::Enriched;
        assert!(!searchable.has_embeddings());
        assert!(enriched.has_embeddings());
    }

    #[test]
    fn enrichment_state_default_is_searchable() {
        assert_eq!(EnrichmentState::default(), EnrichmentState::Searchable);
    }

    #[test]
    fn frame_role_default_is_document() {
        assert_eq!(FrameRole::default(), FrameRole::Document);
    }
}
