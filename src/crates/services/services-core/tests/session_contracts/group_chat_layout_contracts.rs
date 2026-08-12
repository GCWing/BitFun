use bitfun_services_core::session::{validate_room_id, GroupChatStorageLayout};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

struct TestTempDir {
    path: PathBuf,
}

impl TestTempDir {
    fn new(name: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("bitfun-group-chat-layout-{name}-{nonce}"));
        std::fs::create_dir_all(&path).expect("create temp dir");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TestTempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

#[test]
fn group_chat_layout_preserves_contract_file_names() {
    let root = TestTempDir::new("paths");
    let layout = GroupChatStorageLayout::new(root.path().join("group-chats"));
    let room_id = "room-1";

    assert_eq!(
        layout.group_chats_root(),
        root.path().join("group-chats").as_path()
    );
    assert_eq!(
        layout.index_path(),
        root.path().join("group-chats").join("index.json")
    );
    assert_eq!(
        layout.room_dir(room_id),
        root.path().join("group-chats").join("room-1")
    );
    assert_eq!(
        layout.meta_path(room_id),
        root.path()
            .join("group-chats")
            .join("room-1")
            .join("meta.json")
    );
    assert_eq!(
        layout.members_path(room_id),
        root.path()
            .join("group-chats")
            .join("room-1")
            .join("members.json")
    );
    assert_eq!(
        layout.message_catalog_path(room_id),
        root.path()
            .join("group-chats")
            .join("room-1")
            .join("message-catalog.json")
    );
    assert_eq!(
        layout.messages_dir(room_id),
        root.path()
            .join("group-chats")
            .join("room-1")
            .join("messages")
    );
    assert_eq!(
        layout.message_path(room_id, 7),
        root.path()
            .join("group-chats")
            .join("room-1")
            .join("messages")
            .join("message-0007.json")
    );
}

#[test]
fn group_chat_layout_rejects_invalid_room_ids() {
    let root = TestTempDir::new("invalid-room");
    let _layout = GroupChatStorageLayout::new(root.path().join("group-chats"));

    assert!(validate_room_id("").is_err());
    assert!(validate_room_id(".").is_err());
    assert!(validate_room_id("..").is_err());
    assert!(validate_room_id("a/b").is_err());
    assert!(validate_room_id("a\\b").is_err());
    assert!(validate_room_id("c:").is_err());
    assert!(validate_room_id("room\u{0000}1").is_err());

    assert!(validate_room_id("room-1").is_ok());
    assert!(validate_room_id("abc-123").is_ok());
}

#[test]
#[should_panic(expected = "invalid room_id")]
fn group_chat_layout_panics_on_invalid_room_id_path_methods() {
    let root = TestTempDir::new("invalid-room-panic");
    let layout = GroupChatStorageLayout::new(root.path().join("group-chats"));
    let _ = layout.meta_path("../escape");
}

#[tokio::test]
async fn group_chat_layout_lists_indexed_message_paths_in_numeric_order() {
    let root = TestTempDir::new("message-list");
    let layout = GroupChatStorageLayout::new(root.path().join("group-chats"));
    let room_id = "room-1";
    let messages_dir = layout.messages_dir(room_id);
    std::fs::create_dir_all(&messages_dir).expect("messages dir");

    std::fs::write(messages_dir.join("message-0010.json"), "{}").expect("message file");
    std::fs::write(messages_dir.join("message-0002.json"), "{}").expect("message file");
    std::fs::write(messages_dir.join("message-invalid.json"), "{}").expect("ignored file");
    std::fs::write(messages_dir.join("message-0003.txt"), "{}").expect("ignored extension");
    std::fs::write(messages_dir.join("notes.json"), "{}").expect("ignored prefix");

    let indexed_paths = layout
        .list_indexed_message_paths(room_id)
        .await
        .expect("message paths should be listed");

    assert_eq!(
        indexed_paths
            .iter()
            .map(|(index, _)| *index)
            .collect::<Vec<_>>(),
        vec![2, 10]
    );
    assert_eq!(indexed_paths[0].1, layout.message_path(room_id, 2));
    assert_eq!(indexed_paths[1].1, layout.message_path(room_id, 10));
}

#[tokio::test]
async fn group_chat_layout_returns_empty_message_paths_when_messages_dir_is_missing() {
    let root = TestTempDir::new("missing-message-list");
    let layout = GroupChatStorageLayout::new(root.path().join("group-chats"));

    let indexed_paths = layout
        .list_indexed_message_paths("room-1")
        .await
        .expect("missing messages dir should be empty");

    assert!(indexed_paths.is_empty());
}

#[test]
fn group_chat_layout_room_delete_paths_cover_full_cascade() {
    let root = TestTempDir::new("delete-paths");
    let layout = GroupChatStorageLayout::new(root.path().join("group-chats"));
    let room_id = "room-1";

    let delete_paths = layout.room_delete_paths(room_id);

    let expected = vec![
        layout.meta_path(room_id),
        layout.members_path(room_id),
        layout.message_catalog_path(room_id),
        layout.messages_dir(room_id),
        layout.room_dir(room_id),
    ];
    assert_eq!(delete_paths, expected);

    // Every file lives under the room dir, so removing the room dir recursively
    // is sufficient to clear messages and metadata.
    assert!(delete_paths
        .iter()
        .all(|path| path.starts_with(layout.room_dir(room_id))));
    assert!(delete_paths.contains(&layout.room_dir(room_id)));
}
