use std::io::{Read, Seek, SeekFrom, Write};

#[cfg(feature = "lex")]
use std::collections::{HashMap, HashSet};

#[cfg(feature = "lex")]
use crate::types::TantivySegmentDescriptor;
use crate::types::{
    LexSegmentDescriptor, SegmentCommon, TimeSegmentDescriptor, VecSegmentDescriptor,
    VectorCompression,
};
#[cfg(feature = "temporal_track")]
use crate::{
    temporal_track_append, TemporalAnchor, TemporalMention, TEMPORAL_TRACK_FLAG_HAS_ANCHORS,
    TEMPORAL_TRACK_FLAG_HAS_MENTIONS,
};
use crate::{MemvidError, Result};
#[cfg(feature = "temporal_track")]
use std::io::Cursor;

use super::lifecycle::Memvid;

#[cfg(feature = "lex")]
use crate::search::{EmbeddedLexSegment, TantivySnapshot};

#[derive(Debug)]
#[allow(dead_code)]
pub(crate) struct LexSegmentArtifact {
    pub bytes: Vec<u8>,
    pub doc_count: u64,
    pub checksum: [u8; 32],
}

#[derive(Debug)]
#[allow(dead_code)]
pub(crate) struct VecSegmentArtifact {
    pub bytes: Vec<u8>,
    pub vector_count: u64,
    pub dimension: u32,
    pub checksum: [u8; 32],
    pub compression: VectorCompression,
    #[cfg(feature = "parallel_segments")]
    pub bytes_uncompressed: u64,
}

#[derive(Debug)]
#[allow(dead_code)]
pub(crate) struct TimeSegmentArtifact {
    pub bytes: Vec<u8>,
    pub entry_count: u64,
    pub checksum: [u8; 32],
}

#[cfg(feature = "temporal_track")]
#[derive(Debug)]
pub(crate) struct TemporalSegmentArtifact {
    pub bytes: Vec<u8>,
    pub entry_count: u64,
    pub anchor_count: u64,
    pub checksum: [u8; 32],
    pub flags: u32,
}

#[cfg(feature = "lex")]
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) struct TantivySegmentArtifact {
    pub path: String,
    pub bytes: Vec<u8>,
    pub checksum: [u8; 32],
}

#[cfg(feature = "lex")]
#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub(crate) struct TantivySegmentDeltaEntry {
    pub path: String,
    pub existing: Option<TantivySegmentDescriptor>,
    pub artifact: Option<TantivySegmentArtifact>,
}

#[cfg(feature = "lex")]
#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub(crate) struct TantivySnapshotDelta {
    pub doc_count: u64,
    pub checksum: [u8; 32],
    pub entries: Vec<TantivySegmentDeltaEntry>,
    pub removed_paths: Vec<String>,
}

impl Memvid {
    #[allow(dead_code)]
    pub(crate) fn append_lex_segment(
        &mut self,
        artifact: &LexSegmentArtifact,
        segment_id: u64,
    ) -> Result<LexSegmentDescriptor> {
        if artifact.doc_count == 0 || artifact.bytes.is_empty() {
            return Err(MemvidError::CheckpointFailed {
                reason: "lex segment artifact empty".into(),
            });
        }

        let offset = self.data_end;
        let new_end = offset + artifact.bytes.len() as u64;

        // Write at current data_end
        self.file.seek(SeekFrom::Start(offset))?;
        self.file.write_all(&artifact.bytes)?;
        self.file.sync_all()?;
        self.data_end = new_end;

        let common = SegmentCommon::new(
            segment_id,
            offset,
            artifact.bytes.len() as u64,
            artifact.checksum,
        );
        Ok(LexSegmentDescriptor::from_common(
            common,
            artifact.doc_count,
        ))
    }

    #[allow(dead_code)]
    pub(crate) fn append_vec_segment(
        &mut self,
        artifact: &VecSegmentArtifact,
        segment_id: u64,
    ) -> Result<VecSegmentDescriptor> {
        if artifact.vector_count == 0 || artifact.bytes.is_empty() {
            return Err(MemvidError::CheckpointFailed {
                reason: "vec segment artifact empty".into(),
            });
        }

        let offset = self.data_end;
        let new_end = offset + artifact.bytes.len() as u64;

        // Seek to write position
        self.file.seek(SeekFrom::Start(offset))?;

        // Write the actual data
        self.file.write_all(&artifact.bytes)?;
        self.file.sync_all()?;

        // VERIFY: Read back the first few bytes to confirm write persisted
        self.file.seek(SeekFrom::Start(offset))?;
        let mut verify_buf = vec![0u8; 16.min(artifact.bytes.len())];
        self.file.read_exact(&mut verify_buf)?;
        let expected = &artifact.bytes[..verify_buf.len()];
        if verify_buf != expected {
            return Err(MemvidError::CheckpointFailed {
                reason: format!("vec segment write verification failed at offset {offset}"),
            });
        }

        self.data_end = new_end;

        let common = SegmentCommon::new(
            segment_id,
            offset,
            artifact.bytes.len() as u64,
            artifact.checksum,
        );

        tracing::debug!(
            segment_id = common.segment_id,
            artifact_compression = ?artifact.compression,
            vector_count = artifact.vector_count,
            bytes_len = common.bytes_length,
            "created vec segment descriptor"
        );

        Ok(VecSegmentDescriptor::from_common(
            common,
            artifact.vector_count,
            artifact.dimension,
            artifact.compression.clone(),
        ))
    }

    #[cfg(feature = "temporal_track")]
    pub(crate) fn build_temporal_segment_from_records(
        &self,
        mentions: &[TemporalMention],
        anchors: &[TemporalAnchor],
    ) -> Result<Option<TemporalSegmentArtifact>> {
        if mentions.is_empty() && anchors.is_empty() {
            return Ok(None);
        }

        #[cfg(test)]
        println!(
            "build_temporal_segment_from_records: mentions={}, anchors={}",
            mentions.len(),
            anchors.len()
        );

        let mut mention_vec = mentions.to_vec();
        let mut anchor_vec = anchors.to_vec();
        mention_vec.sort_by_key(|m| (m.ts_utc, m.frame_id, m.byte_start));
        anchor_vec.sort_by_key(|a| a.frame_id);

        let mut flags = 0;
        if !anchor_vec.is_empty() {
            flags |= TEMPORAL_TRACK_FLAG_HAS_ANCHORS;
        }
        if !mention_vec.is_empty() {
            flags |= TEMPORAL_TRACK_FLAG_HAS_MENTIONS;
        }

        let mut cursor = Cursor::new(Vec::new());
        let (_, _length, checksum) =
            temporal_track_append(&mut cursor, &mut mention_vec, &mut anchor_vec, flags)?;
        let bytes = cursor.into_inner();
        if bytes.is_empty() {
            return Ok(None);
        }

        Ok(Some(TemporalSegmentArtifact {
            bytes,
            entry_count: mention_vec.len() as u64,
            anchor_count: anchor_vec.len() as u64,
            checksum,
            flags,
        }))
    }

    #[allow(dead_code)]
    pub(crate) fn append_time_segment(
        &mut self,
        artifact: &TimeSegmentArtifact,
        segment_id: u64,
    ) -> Result<TimeSegmentDescriptor> {
        if artifact.entry_count == 0 || artifact.bytes.is_empty() {
            return Err(MemvidError::CheckpointFailed {
                reason: "time segment artifact empty".into(),
            });
        }

        let offset = self.data_end;
        let new_end = offset + artifact.bytes.len() as u64;

        // Write at current data_end
        self.file.seek(SeekFrom::Start(offset))?;
        self.file.write_all(&artifact.bytes)?;
        self.file.sync_all()?;
        self.data_end = new_end;

        let common = SegmentCommon::new(
            segment_id,
            offset,
            artifact.bytes.len() as u64,
            artifact.checksum,
        );
        Ok(TimeSegmentDescriptor::from_common(
            common,
            artifact.entry_count,
        ))
    }

    #[cfg(feature = "temporal_track")]
    pub(crate) fn append_temporal_segment(
        &mut self,
        artifact: &TemporalSegmentArtifact,
        segment_id: u64,
    ) -> Result<crate::types::TemporalSegmentDescriptor> {
        if artifact.entry_count == 0 && artifact.anchor_count == 0 {
            return Err(MemvidError::CheckpointFailed {
                reason: "temporal segment artifact empty".into(),
            });
        }

        let offset = self.data_end;
        self.file.seek(SeekFrom::Start(offset))?;
        self.file.write_all(&artifact.bytes)?;
        self.file.flush()?;
        self.data_end = offset + artifact.bytes.len() as u64;

        let common = SegmentCommon::new(
            segment_id,
            offset,
            artifact.bytes.len() as u64,
            artifact.checksum,
        );
        Ok(crate::types::TemporalSegmentDescriptor::from_common(
            common,
            artifact.entry_count,
            artifact.anchor_count,
            artifact.flags,
        ))
    }

    #[cfg(feature = "lex")]
    #[allow(dead_code)]
    pub(crate) fn append_tantivy_segment(
        &mut self,
        artifact: &TantivySegmentArtifact,
        segment_id: u64,
    ) -> Result<TantivySegmentDescriptor> {
        if artifact.bytes.is_empty() {
            return Err(MemvidError::CheckpointFailed {
                reason: format!("tantivy segment artifact '{}' empty", artifact.path),
            });
        }

        let offset = self.data_end;
        self.file.seek(SeekFrom::Start(offset))?;
        self.file.write_all(&artifact.bytes)?;
        self.file.flush()?;
        self.data_end = offset + artifact.bytes.len() as u64;

        let common = SegmentCommon::new(
            segment_id,
            offset,
            artifact.bytes.len() as u64,
            artifact.checksum,
        );
        Ok(TantivySegmentDescriptor::from_common(
            common,
            artifact.path.clone(),
        ))
    }

    #[cfg(feature = "lex")]
    pub(crate) fn derive_tantivy_snapshot_delta(
        &self,
        snapshot: TantivySnapshot,
    ) -> TantivySnapshotDelta {
        let mut latest: HashMap<String, &TantivySegmentDescriptor> = HashMap::new();
        for descriptor in &self.toc.segment_catalog.tantivy_segments {
            latest
                .entry(descriptor.path.clone())
                .and_modify(|existing| {
                    if descriptor.common.segment_id > existing.common.segment_id {
                        *existing = descriptor;
                    }
                })
                .or_insert(descriptor);
        }

        let mut entries = Vec::with_capacity(snapshot.segments.len());
        let mut present_paths: HashSet<String> = HashSet::with_capacity(snapshot.segments.len());

        for blob in snapshot.segments {
            let path = blob.path.clone();
            let existing = latest
                .get(path.as_str())
                .map(|descriptor| (*descriptor).clone());
            let requires_append = existing
                .as_ref()
                .is_none_or(|descriptor| descriptor.common.checksum != blob.checksum);

            let artifact = if requires_append {
                Some(TantivySegmentArtifact {
                    path: path.clone(),
                    bytes: blob.bytes,
                    checksum: blob.checksum,
                })
            } else {
                None
            };

            entries.push(TantivySegmentDeltaEntry {
                path: path.clone(),
                existing,
                artifact,
            });
            present_paths.insert(path);
        }

        let removed_paths = latest
            .keys()
            .filter(|path| !present_paths.contains(path.as_str()))
            .cloned()
            .collect();

        TantivySnapshotDelta {
            doc_count: snapshot.doc_count,
            checksum: snapshot.checksum,
            entries,
            removed_paths,
        }
    }

    #[cfg(feature = "lex")]
    #[allow(dead_code)]
    pub(crate) fn publish_tantivy_delta(&mut self) -> Result<bool> {
        let engine = match self.tantivy.as_mut() {
            Some(engine) => engine,
            None => return Ok(false),
        };

        let snapshot = engine.snapshot_segments()?;
        let delta = self.derive_tantivy_snapshot_delta(snapshot);

        let mut active_descriptors: HashMap<String, TantivySegmentDescriptor> = self
            .toc
            .segment_catalog
            .tantivy_segments
            .iter()
            .map(|descriptor| (descriptor.path.clone(), descriptor.clone()))
            .collect();

        let mut next_segment_id = self.toc.segment_catalog.next_segment_id;
        let initial_offset = self.data_end;
        let mut changed = false;

        for entry in delta.entries {
            if let Some(artifact) = entry.artifact {
                if artifact.bytes.is_empty() {
                    continue;
                }
                let descriptor = match self.append_tantivy_segment(&artifact, next_segment_id) {
                    Ok(descriptor) => descriptor,
                    Err(err) => {
                        self.data_end = initial_offset;
                        self.file.set_len(initial_offset)?;
                        return Err(err);
                    }
                };
                next_segment_id = next_segment_id.saturating_add(1);
                active_descriptors.insert(entry.path.clone(), descriptor);
                changed = true;
            } else if let Some(existing) = entry.existing {
                active_descriptors
                    .entry(entry.path.clone())
                    .or_insert(existing);
            }
        }

        for path in delta.removed_paths {
            if active_descriptors.remove(&path).is_some() {
                changed = true;
            }
        }

        let current_doc_count = self
            .toc
            .indexes
            .lex
            .as_ref()
            .map_or(0, |manifest| manifest.doc_count);
        if current_doc_count != delta.doc_count {
            changed = true;
        }

        let current_checksum = self
            .toc
            .indexes
            .lex
            .as_ref()
            .map_or([0u8; 32], |manifest| manifest.checksum);
        if current_checksum != delta.checksum {
            changed = true;
        }

        if !changed {
            return Ok(false);
        }

        let mut descriptors: Vec<TantivySegmentDescriptor> =
            active_descriptors.into_values().collect();
        descriptors.sort_by_key(|descriptor| descriptor.common.segment_id);

        let embedded_segments: Vec<EmbeddedLexSegment> = descriptors
            .iter()
            .map(|descriptor| EmbeddedLexSegment {
                path: descriptor.path.clone(),
                bytes_offset: descriptor.common.bytes_offset,
                bytes_length: descriptor.common.bytes_length,
                checksum: descriptor.common.checksum,
            })
            .collect();

        let previous_manifest = self.toc.indexes.lex.clone();
        let (index_manifest, manifest_segments) = {
            let mut storage = self.lex_storage.write().map_err(|_| MemvidError::Tantivy {
                reason: "embedded lex storage lock poisoned".into(),
            })?;
            storage.replace(delta.doc_count, delta.checksum, embedded_segments.clone());
            storage.to_manifest()
        };

        if let Some(mut storage_manifest) = index_manifest {
            if storage_manifest.bytes_offset == 0 && storage_manifest.bytes_length == 0 {
                if let Some(prev) = previous_manifest.as_ref() {
                    storage_manifest.bytes_offset = prev.bytes_offset;
                    storage_manifest.bytes_length = prev.bytes_length;
                }
            }
            if let Some(existing) = self.toc.indexes.lex.as_mut() {
                existing.doc_count = storage_manifest.doc_count;
                existing.generation = storage_manifest.generation;
                existing.checksum = storage_manifest.checksum;
                if existing.bytes_length == 0 && storage_manifest.bytes_length != 0 {
                    existing.bytes_offset = storage_manifest.bytes_offset;
                    existing.bytes_length = storage_manifest.bytes_length;
                }
            } else {
                self.toc.indexes.lex = Some(storage_manifest);
            }
        } else {
            self.toc.indexes.lex = None;
        }
        self.toc.indexes.lex_segments = manifest_segments;

        self.toc.segment_catalog.tantivy_segments = descriptors;
        self.toc.segment_catalog.next_segment_id = next_segment_id;
        self.toc.segment_catalog.version = self.toc.segment_catalog.version.max(1);

        Ok(true)
    }
}
