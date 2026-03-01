#![allow(dead_code)]

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::types::{LexIndexManifest, LexSegmentManifest};

/// Placeholder embedded segment metadata for future Tantivy integration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EmbeddedLexSegment {
    pub path: String,
    pub bytes_offset: u64,
    pub bytes_length: u64,
    pub checksum: [u8; 32],
}

impl EmbeddedLexSegment {
    pub fn from_manifest(manifest: &LexSegmentManifest) -> Self {
        Self {
            path: manifest.path.clone(),
            bytes_offset: manifest.bytes_offset,
            bytes_length: manifest.bytes_length,
            checksum: manifest.checksum,
        }
    }

    pub fn to_manifest(&self) -> LexSegmentManifest {
        LexSegmentManifest {
            path: self.path.clone(),
            bytes_offset: self.bytes_offset,
            bytes_length: self.bytes_length,
            checksum: self.checksum,
        }
    }
}

/// In-memory representation of embedded Tantivy state.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EmbeddedLexStorage {
    generation: u64,
    doc_count: u64,
    checksum: [u8; 32],
    segments: BTreeMap<String, EmbeddedLexSegment>,
}

impl EmbeddedLexStorage {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_manifest(
        manifest: Option<&LexIndexManifest>,
        segments: &[LexSegmentManifest],
    ) -> Self {
        let mut storage = Self::new();
        if let Some(index) = manifest {
            storage.generation = index.generation;
            storage.doc_count = index.doc_count;
            storage.checksum = index.checksum;
        }
        for segment in segments {
            storage.segments.insert(
                segment.path.clone(),
                EmbeddedLexSegment::from_manifest(segment),
            );
        }
        storage
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn doc_count(&self) -> u64 {
        self.doc_count
    }

    pub fn checksum(&self) -> [u8; 32] {
        self.checksum
    }

    pub fn segments(&self) -> impl Iterator<Item = &EmbeddedLexSegment> {
        self.segments.values()
    }

    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }

    pub fn replace(
        &mut self,
        doc_count: u64,
        checksum: [u8; 32],
        segments: Vec<EmbeddedLexSegment>,
    ) {
        self.doc_count = doc_count;
        self.checksum = checksum;
        self.segments.clear();
        for segment in segments {
            self.segments.insert(segment.path.clone(), segment);
        }
        self.generation = self.generation.wrapping_add(1);
    }

    pub fn clear(&mut self) {
        self.replace(0, [0u8; 32], Vec::new());
    }

    pub fn insert(&mut self, segment: EmbeddedLexSegment) {
        self.segments.insert(segment.path.clone(), segment);
    }

    pub fn remove(&mut self, path: &str) {
        self.segments.remove(path);
    }

    pub fn set_generation(&mut self, generation: u64) {
        self.generation = generation;
    }

    pub fn set_doc_count(&mut self, doc_count: u64) {
        self.doc_count = doc_count;
    }

    pub fn set_checksum(&mut self, checksum: [u8; 32]) {
        self.checksum = checksum;
    }

    pub fn to_manifest(&self) -> (Option<LexIndexManifest>, Vec<LexSegmentManifest>) {
        let index_manifest = None;

        let segments = self
            .segments
            .values()
            .map(EmbeddedLexSegment::to_manifest)
            .collect();
        (index_manifest, segments)
    }

    pub fn adjust_offsets(&mut self, delta: u64) {
        if delta == 0 {
            return;
        }
        for segment in self.segments.values_mut() {
            if segment.bytes_offset != 0 {
                segment.bytes_offset += delta;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_segment(path: &str, offset: u64, length: u64) -> EmbeddedLexSegment {
        EmbeddedLexSegment {
            path: path.to_string(),
            bytes_offset: offset,
            bytes_length: length,
            checksum: [0u8; 32],
        }
    }

    #[test]
    fn new_storage_is_empty() {
        let storage = EmbeddedLexStorage::new();
        assert!(storage.is_empty());
        assert_eq!(storage.doc_count(), 0);
        assert_eq!(storage.generation(), 0);
        assert_eq!(storage.checksum(), [0u8; 32]);
        assert_eq!(storage.segments().count(), 0);
    }

    #[test]
    fn insert_adds_segment() {
        let mut storage = EmbeddedLexStorage::new();
        let segment = make_segment("seg_001.bin", 100, 200);
        storage.insert(segment);

        assert!(!storage.is_empty());
        assert_eq!(storage.segments().count(), 1);
        let first = storage.segments().next().unwrap();
        assert_eq!(first.path, "seg_001.bin");
        assert_eq!(first.bytes_offset, 100);
        assert_eq!(first.bytes_length, 200);
    }

    #[test]
    fn insert_overwrites_same_path() {
        let mut storage = EmbeddedLexStorage::new();
        storage.insert(make_segment("seg_001.bin", 100, 200));
        storage.insert(make_segment("seg_001.bin", 300, 400));

        assert_eq!(storage.segments().count(), 1);
        let first = storage.segments().next().unwrap();
        assert_eq!(first.bytes_offset, 300);
        assert_eq!(first.bytes_length, 400);
    }

    #[test]
    fn remove_deletes_segment() {
        let mut storage = EmbeddedLexStorage::new();
        storage.insert(make_segment("seg_001.bin", 100, 200));
        storage.insert(make_segment("seg_002.bin", 300, 400));
        assert_eq!(storage.segments().count(), 2);

        storage.remove("seg_001.bin");
        assert_eq!(storage.segments().count(), 1);
        let remaining = storage.segments().next().unwrap();
        assert_eq!(remaining.path, "seg_002.bin");
    }

    #[test]
    fn remove_nonexistent_is_noop() {
        let mut storage = EmbeddedLexStorage::new();
        storage.insert(make_segment("seg_001.bin", 100, 200));
        storage.remove("nonexistent.bin");
        assert_eq!(storage.segments().count(), 1);
    }

    #[test]
    fn replace_clears_and_sets_new_segments() {
        let mut storage = EmbeddedLexStorage::new();
        storage.insert(make_segment("old.bin", 10, 20));
        let initial_gen = storage.generation();

        let new_segments = vec![
            make_segment("new_a.bin", 100, 200),
            make_segment("new_b.bin", 300, 400),
        ];
        let checksum = [42u8; 32];
        storage.replace(5, checksum, new_segments);

        assert_eq!(storage.doc_count(), 5);
        assert_eq!(storage.checksum(), checksum);
        assert_eq!(storage.segments().count(), 2);
        assert_eq!(storage.generation(), initial_gen.wrapping_add(1));
    }

    #[test]
    fn clear_resets_all_state() {
        let mut storage = EmbeddedLexStorage::new();
        storage.insert(make_segment("seg.bin", 100, 200));
        storage.set_doc_count(10);
        storage.set_checksum([1u8; 32]);
        let gen_before_clear = storage.generation();

        storage.clear();

        assert!(storage.is_empty());
        assert_eq!(storage.doc_count(), 0);
        assert_eq!(storage.checksum(), [0u8; 32]);
        // clear calls replace which increments generation
        assert_eq!(storage.generation(), gen_before_clear.wrapping_add(1));
    }

    #[test]
    fn adjust_offsets_shifts_nonzero_offsets() {
        let mut storage = EmbeddedLexStorage::new();
        storage.insert(make_segment("a.bin", 0, 100));
        storage.insert(make_segment("b.bin", 200, 100));
        storage.insert(make_segment("c.bin", 500, 100));

        storage.adjust_offsets(50);

        let segments: Vec<_> = storage.segments().collect();
        // Segment at offset 0 should NOT be shifted
        let seg_a = segments.iter().find(|s| s.path == "a.bin").unwrap();
        assert_eq!(seg_a.bytes_offset, 0);
        // Non-zero offsets should be shifted by delta
        let seg_b = segments.iter().find(|s| s.path == "b.bin").unwrap();
        assert_eq!(seg_b.bytes_offset, 250);
        let seg_c = segments.iter().find(|s| s.path == "c.bin").unwrap();
        assert_eq!(seg_c.bytes_offset, 550);
    }

    #[test]
    fn adjust_offsets_zero_delta_is_noop() {
        let mut storage = EmbeddedLexStorage::new();
        storage.insert(make_segment("a.bin", 200, 100));
        storage.adjust_offsets(0);

        let seg = storage.segments().next().unwrap();
        assert_eq!(seg.bytes_offset, 200);
    }

    #[test]
    fn set_generation_and_doc_count() {
        let mut storage = EmbeddedLexStorage::new();
        storage.set_generation(42);
        storage.set_doc_count(100);
        storage.set_checksum([7u8; 32]);

        assert_eq!(storage.generation(), 42);
        assert_eq!(storage.doc_count(), 100);
        assert_eq!(storage.checksum(), [7u8; 32]);
    }

    #[test]
    fn to_manifest_returns_segment_manifests() {
        let mut storage = EmbeddedLexStorage::new();
        storage.insert(make_segment("seg_a.bin", 100, 200));
        storage.insert(make_segment("seg_b.bin", 300, 400));

        let (_index_manifest, segment_manifests) = storage.to_manifest();
        assert_eq!(segment_manifests.len(), 2);
        // BTreeMap produces sorted keys
        assert_eq!(segment_manifests[0].path, "seg_a.bin");
        assert_eq!(segment_manifests[1].path, "seg_b.bin");
    }

    #[test]
    fn from_manifest_round_trip() {
        let segment_manifests = vec![
            LexSegmentManifest {
                path: "seg_001.bin".to_string(),
                bytes_offset: 100,
                bytes_length: 200,
                checksum: [1u8; 32],
            },
            LexSegmentManifest {
                path: "seg_002.bin".to_string(),
                bytes_offset: 300,
                bytes_length: 400,
                checksum: [2u8; 32],
            },
        ];
        let index_manifest = LexIndexManifest {
            doc_count: 42,
            generation: 7,
            bytes_offset: 0,
            bytes_length: 0,
            checksum: [99u8; 32],
        };

        let storage = EmbeddedLexStorage::from_manifest(Some(&index_manifest), &segment_manifests);
        assert_eq!(storage.generation(), 7);
        assert_eq!(storage.doc_count(), 42);
        assert_eq!(storage.segments().count(), 2);
    }

    #[test]
    fn segment_from_manifest_and_back() {
        let manifest = LexSegmentManifest {
            path: "test.bin".to_string(),
            bytes_offset: 42,
            bytes_length: 128,
            checksum: [5u8; 32],
        };
        let segment = EmbeddedLexSegment::from_manifest(&manifest);
        assert_eq!(segment.path, "test.bin");
        assert_eq!(segment.bytes_offset, 42);
        assert_eq!(segment.bytes_length, 128);
        assert_eq!(segment.checksum, [5u8; 32]);

        let back = segment.to_manifest();
        assert_eq!(back.path, manifest.path);
        assert_eq!(back.bytes_offset, manifest.bytes_offset);
        assert_eq!(back.bytes_length, manifest.bytes_length);
        assert_eq!(back.checksum, manifest.checksum);
    }
}
