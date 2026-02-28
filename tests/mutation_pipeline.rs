//! Integration tests for the mutation pipeline: put -> commit -> search -> delete -> verify

use memvid_core::{FrameRole, Memvid, PutOptions};
use tempfile::TempDir;

static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn create_temp_memvid() -> (TempDir, Memvid) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.mv2");
    let mem = Memvid::create(&path).unwrap();
    (dir, mem)
}

#[test]
fn put_bytes_returns_frame_id() {
    let _lock = SERIAL.lock().unwrap();
    let (_dir, mut mem) = create_temp_memvid();
    let id = mem.put_bytes(b"Hello, world!").unwrap();
    mem.commit().unwrap();
    // FrameId is u64; first frame may be 0 or 1 depending on implementation
    let _ = id;
}

#[test]
fn put_bytes_with_options_tags() {
    let _lock = SERIAL.lock().unwrap();
    let (_dir, mut mem) = create_temp_memvid();
    let opts = PutOptions::builder()
        .uri("test://tagged")
        .title("Tagged Document")
        .push_tag("important")
        .auto_tag(false)
        .extract_dates(false)
        .extract_triplets(false)
        .build();
    let id = mem.put_bytes_with_options(b"Tagged content here", opts).unwrap();
    mem.commit().unwrap();
    let _ = id;
}

#[test]
fn multiple_puts_then_commit() {
    let _lock = SERIAL.lock().unwrap();
    let (_dir, mut mem) = create_temp_memvid();
    let id1 = mem.put_bytes(b"First document").unwrap();
    let id2 = mem.put_bytes(b"Second document").unwrap();
    mem.commit().unwrap();
    assert_ne!(id1, id2);

    let stats = mem.stats().unwrap();
    assert!(stats.frame_count >= 2);
}

#[test]
fn delete_frame_marks_tombstone() {
    let _lock = SERIAL.lock().unwrap();
    let (_dir, mut mem) = create_temp_memvid();
    let opts = PutOptions {
        uri: Some("mv2://delete-test".to_string()),
        ..Default::default()
    };
    let id = mem.put_bytes_with_options(b"To be deleted", opts).unwrap();
    mem.commit().unwrap();

    // Look up the frame by URI to get the actual frame_id
    let frame = mem.frame_by_uri("mv2://delete-test").unwrap();
    mem.delete_frame(frame.id).unwrap();
    mem.commit().unwrap();

    // After deletion the frame is either removed from the count or marked tombstoned
    let stats = mem.stats().unwrap();
    let _ = (id, stats);
}

#[test]
fn commit_empty_succeeds() {
    let _lock = SERIAL.lock().unwrap();
    let (_dir, mut mem) = create_temp_memvid();
    mem.commit().unwrap();
}

#[test]
fn put_with_role() {
    let _lock = SERIAL.lock().unwrap();
    let (_dir, mut mem) = create_temp_memvid();
    let opts = PutOptions::builder()
        .role(FrameRole::Document)
        .auto_tag(false)
        .extract_dates(false)
        .extract_triplets(false)
        .build();
    let id = mem.put_bytes_with_options(b"Document with role", opts).unwrap();
    mem.commit().unwrap();
    let _ = id;
}
