use std::borrow::Cow;
use std::path::PathBuf;

use thiserror::Error;

/// Result alias used throughout the core crate.
pub type Result<T> = std::result::Result<T, MemvidError>;

/// Process metadata for a lock holder used to produce human readable diagnostics.
#[derive(Debug, Clone)]
pub struct LockOwnerHint {
    pub pid: Option<u32>,
    pub cmd: Option<String>,
    pub started_at: Option<String>,
    pub file_path: Option<PathBuf>,
    pub file_id: Option<String>,
    pub last_heartbeat: Option<String>,
    pub heartbeat_ms: Option<u64>,
}

/// Structured error returned when a `.mv2` is locked by another writer.
#[derive(Debug, Error, Clone)]
#[error("{message}")]
pub struct LockedError {
    pub file: PathBuf,
    pub message: String,
    pub owner: Option<LockOwnerHint>,
    pub stale: bool,
}

impl LockedError {
    #[must_use]
    pub fn new(
        file: PathBuf,
        message: impl Into<String>,
        owner: Option<LockOwnerHint>,
        stale: bool,
    ) -> Self {
        Self {
            file,
            message: message.into(),
            owner,
            stale,
        }
    }
}

/// Canonical error surface for memvid-core.
#[derive(Debug, Error)]
pub enum MemvidError {
    #[error("I/O error: {source}")]
    Io {
        source: std::io::Error,
        path: Option<PathBuf>,
    },

    #[error("Serialization error: {0}")]
    Encode(#[from] bincode::error::EncodeError),

    #[error("Deserialization error: {0}")]
    Decode(#[from] bincode::error::DecodeError),

    #[error("Lock acquisition failed: {0}")]
    Lock(String),

    #[error(transparent)]
    Locked(#[from] Box<LockedError>),

    #[error("Checksum mismatch while validating {context}")]
    ChecksumMismatch { context: &'static str },

    #[error("Header validation failed: {reason}")]
    InvalidHeader { reason: Cow<'static, str> },

    #[error("This file is encrypted: {path}\n{hint}")]
    EncryptedFile { path: PathBuf, hint: String },

    #[error("Table of contents validation failed: {reason}")]
    InvalidToc { reason: Cow<'static, str> },

    #[error("Time index track is invalid: {reason}")]
    InvalidTimeIndex { reason: Cow<'static, str> },

    #[error("Sketch track is invalid: {reason}")]
    InvalidSketchTrack { reason: Cow<'static, str> },

    #[cfg(feature = "temporal_track")]
    #[error("Temporal track is invalid: {reason}")]
    InvalidTemporalTrack { reason: Cow<'static, str> },

    #[error("Logic-Mesh is invalid: {reason}")]
    InvalidLogicMesh { reason: Cow<'static, str> },

    #[error("Logic-Mesh is not enabled")]
    LogicMeshNotEnabled,

    #[error("NER model not available: {reason}")]
    NerModelNotAvailable { reason: Cow<'static, str> },

    #[error("Unsupported tier requested")]
    InvalidTier,

    #[error("Lexical index is not enabled")]
    LexNotEnabled,

    #[error("Vector index is not enabled")]
    VecNotEnabled,

    #[error("CLIP index is not enabled")]
    ClipNotEnabled,

    #[error("Vector dimension mismatch (expected {expected}, got {actual})")]
    VecDimensionMismatch { expected: u32, actual: usize },

    #[error("Auxiliary file detected: {path:?}")]
    AuxiliaryFileDetected { path: PathBuf },

    #[error("Embedded WAL is corrupted at offset {offset}: {reason}")]
    WalCorruption {
        offset: u64,
        reason: Cow<'static, str>,
    },

    #[error("Manifest WAL is corrupted at offset {offset}: {reason}")]
    ManifestWalCorrupted { offset: u64, reason: &'static str },

    #[error("Unable to checkpoint embedded WAL: {reason}")]
    CheckpointFailed { reason: String },

    #[error("Ticket sequence is out of order (expected > {expected}, got {actual})")]
    TicketSequence { expected: i64, actual: i64 },

    #[error("Apply a ticket before mutating this memory (tier {tier:?})")]
    TicketRequired { tier: crate::types::Tier },

    #[error(
        "Capacity exceeded. Current: {current} bytes, Limit: {limit} bytes, Required: {required} bytes"
    )]
    CapacityExceeded {
        current: u64,
        limit: u64,
        required: u64,
    },

    #[error("API key required for files larger than {limit} bytes. File size: {file_size} bytes")]
    ApiKeyRequired { file_size: u64, limit: u64 },

    #[error(
        "Memory already bound to '{existing_memory_name}' ({existing_memory_id}). Bound at: {bound_at}"
    )]
    MemoryAlreadyBound {
        existing_memory_id: uuid::Uuid,
        existing_memory_name: String,
        bound_at: String,
    },

    #[error("Operation requires a sealed memory")]
    RequiresSealed,

    #[error("Operation requires an open memory")]
    RequiresOpen,

    #[error("Doctor command requires at least one operation")]
    DoctorNoOp,

    #[error("Doctor operation failed: {reason}")]
    Doctor { reason: String },

    #[error("Feature '{feature}' is not available in this build")]
    FeatureUnavailable { feature: &'static str },

    #[error("Invalid search cursor: {reason}")]
    InvalidCursor { reason: &'static str },

    #[error("Invalid frame {frame_id}: {reason}")]
    InvalidFrame {
        frame_id: crate::types::FrameId,
        reason: &'static str,
    },

    #[error("Frame {frame_id} was not found")]
    FrameNotFound { frame_id: crate::types::FrameId },

    #[error("Frame with uri '{uri}' was not found")]
    FrameNotFoundByUri { uri: String },

    #[error("Ticket signature verification failed: {reason}")]
    TicketSignatureInvalid { reason: Box<str> },

    #[error("Model signature verification failed: {reason}")]
    ModelSignatureInvalid { reason: Box<str> },

    #[error("Model manifest invalid: {reason}")]
    ModelManifestInvalid { reason: Box<str> },

    #[error("Model integrity check failed: {reason}")]
    ModelIntegrity { reason: Box<str> },

    #[error("Extraction failed: {reason}")]
    ExtractionFailed { reason: Box<str> },

    #[error("Embedding failed: {reason}")]
    EmbeddingFailed { reason: Box<str> },

    #[error("Reranking failed: {reason}")]
    RerankFailed { reason: Box<str> },

    #[error("Model mismatch: Index is bound to '{expected}', but requested model was '{actual}'")]
    ModelMismatch { expected: String, actual: String },

    #[error("Invalid query: {reason}")]
    InvalidQuery { reason: String },

    #[error("Tantivy error: {reason}")]
    Tantivy { reason: String },

    #[error("Table extraction failed: {reason}")]
    TableExtraction { reason: String },

    #[error("Schema validation failed: {reason}")]
    SchemaValidation { reason: String },
}

impl From<std::io::Error> for MemvidError {
    fn from(source: std::io::Error) -> Self {
        Self::Io { source, path: None }
    }
}

#[cfg(feature = "lex")]
impl From<tantivy::TantivyError> for MemvidError {
    fn from(value: tantivy::TantivyError) -> Self {
        Self::Tantivy {
            reason: value.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    #[test]
    fn io_error_display_is_non_empty() {
        let err = MemvidError::Io {
            source: io::Error::new(io::ErrorKind::NotFound, "file missing"),
            path: None,
        };
        let msg = format!("{err}");
        assert!(!msg.is_empty());
        assert!(msg.contains("I/O error"));
    }

    #[test]
    fn checksum_mismatch_display_is_non_empty() {
        let err = MemvidError::ChecksumMismatch {
            context: "header",
        };
        let msg = format!("{err}");
        assert!(!msg.is_empty());
        assert!(msg.contains("Checksum mismatch"));
    }

    #[test]
    fn invalid_header_display_is_non_empty() {
        let err = MemvidError::InvalidHeader {
            reason: "bad magic".into(),
        };
        let msg = format!("{err}");
        assert!(!msg.is_empty());
        assert!(msg.contains("Header validation failed"));
    }

    #[test]
    fn locked_error_display_is_non_empty() {
        let locked = LockedError::new(
            PathBuf::from("/tmp/test.mv2"),
            "locked by another process",
            None,
            false,
        );
        let err = MemvidError::Locked(Box::new(locked));
        let msg = format!("{err}");
        assert!(!msg.is_empty());
        assert!(msg.contains("locked"));
    }

    #[test]
    fn from_io_error_conversion() {
        let io_err = io::Error::new(io::ErrorKind::PermissionDenied, "access denied");
        let memvid_err: MemvidError = io_err.into();
        match memvid_err {
            MemvidError::Io { source, path } => {
                assert_eq!(source.kind(), io::ErrorKind::PermissionDenied);
                assert!(path.is_none());
            }
            other => panic!("Expected MemvidError::Io, got: {other:?}"),
        }
    }

    #[test]
    fn lock_owner_hint_construction() {
        let hint = LockOwnerHint {
            pid: Some(12345),
            cmd: Some("memvid build".to_string()),
            started_at: Some("2026-01-01T00:00:00Z".to_string()),
            file_path: Some(PathBuf::from("/tmp/test.mv2")),
            file_id: Some("abc-123".to_string()),
            last_heartbeat: Some("2026-01-01T00:00:01Z".to_string()),
            heartbeat_ms: Some(2000),
        };
        assert_eq!(hint.pid, Some(12345));
        assert_eq!(hint.cmd.as_deref(), Some("memvid build"));
        assert!(hint.file_path.is_some());
    }

    #[test]
    fn locked_error_new_with_owner_hint() {
        let hint = LockOwnerHint {
            pid: Some(999),
            cmd: None,
            started_at: None,
            file_path: None,
            file_id: None,
            last_heartbeat: None,
            heartbeat_ms: None,
        };
        let err = LockedError::new(
            PathBuf::from("/data/file.mv2"),
            "locked by pid 999",
            Some(hint),
            true,
        );
        assert!(err.stale);
        assert!(err.owner.is_some());
        assert_eq!(err.file, PathBuf::from("/data/file.mv2"));
    }

    #[test]
    fn various_error_variants_have_non_empty_display() {
        let errors: Vec<MemvidError> = vec![
            MemvidError::Lock("test lock".to_string()),
            MemvidError::LexNotEnabled,
            MemvidError::VecNotEnabled,
            MemvidError::ClipNotEnabled,
            MemvidError::InvalidTier,
            MemvidError::RequiresSealed,
            MemvidError::RequiresOpen,
            MemvidError::DoctorNoOp,
            MemvidError::ExtractionFailed {
                reason: "test".into(),
            },
            MemvidError::EmbeddingFailed {
                reason: "test".into(),
            },
        ];
        for err in &errors {
            let msg = format!("{err}");
            assert!(!msg.is_empty(), "Empty display for error: {err:?}");
        }
    }
}
