use bitfun_runtime_ports::{
    GroupChatActor, GroupChatMember, GroupChatMemberRole, GroupChatMessage, GroupChatMessageKind,
    GroupChatMessageStatus, GroupChatMode, GroupChatRoom, GroupChatStatus,
};
use bitfun_services_core::session::{
    GroupChatStore, GroupChatStoreError, StoredGroupChatMessageCatalog,
};
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
        let path = std::env::temp_dir().join(format!("bitfun-group-chat-store-{name}-{nonce}"));
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

fn sample_room(room_id: &str, name: &str) -> GroupChatRoom {
    GroupChatRoom {
        schema_version: 1,
        room_id: room_id.to_string(),
        name: name.to_string(),
        owner: GroupChatActor::Master,
        mode: GroupChatMode::Free,
        round_robin_cursor: 0,
        created_at: 1,
        last_active_at: 1,
        status: GroupChatStatus::Active,
        member_limit: 50,
        members: Vec::new(),
    }
}

fn sample_members(room_id: &str) -> Vec<GroupChatMember> {
    vec![GroupChatMember {
        session_id: format!("{room_id}-member-1"),
        role: GroupChatMemberRole::Member,
        joined_at: 1,
        agent_type: "Claw".to_string(),
        display_name: Some("Assistant One".to_string()),
    }]
}

fn sample_message(room_id: &str, message_id: &str, content: &str) -> GroupChatMessage {
    GroupChatMessage {
        message_id: message_id.to_string(),
        room_id: room_id.to_string(),
        author: GroupChatActor::Master,
        kind: GroupChatMessageKind::User,
        content: content.to_string(),
        mention_targets: Vec::new(),
        reply_to_message_id: None,
        timestamp: 1,
        status: GroupChatMessageStatus::Pending,
    }
}

#[tokio::test]
async fn group_chat_store_write_read_round_trip_survives_reopen() {
    let root = TestTempDir::new("roundtrip");
    let store = GroupChatStore::new(root.path().join("group-chats"));

    let room = sample_room("room-1", "Test Room");
    store.save_room(&room).await.expect("save room");
    store
        .save_members("room-1", &sample_members("room-1"))
        .await
        .expect("save members");

    let message = sample_message("room-1", "msg-1", "hello group");
    let index = store
        .append_message("room-1", &message)
        .await
        .expect("append message");
    assert_eq!(index, 0);

    // Simulate restart: a fresh store instance reads the same files.
    let reopened = GroupChatStore::new(root.path().join("group-chats"));
    let loaded = reopened.load_room("room-1").await.expect("load room");
    assert_eq!(loaded.room_id, "room-1");
    assert_eq!(loaded.name, "Test Room");
    assert_eq!(loaded.members.len(), 1);
    assert_eq!(loaded.members[0].session_id, "room-1-member-1");
    assert_eq!(loaded.members[0].agent_type, "Claw");

    let window = reopened
        .list_messages("room-1", None, None)
        .await
        .expect("list messages");
    assert_eq!(window.messages.len(), 1);
    assert_eq!(window.messages[0].message_id, "msg-1");
    assert_eq!(window.messages[0].content, "hello group");
    assert_eq!(window.messages[0].status, GroupChatMessageStatus::Pending);
}

#[tokio::test]
async fn group_chat_store_members_are_read_from_members_json_not_meta() {
    let root = TestTempDir::new("members-single-source");
    let store = GroupChatStore::new(root.path().join("group-chats"));

    let room = sample_room("room-2", "Members Source");
    store.save_room(&room).await.expect("save room");
    store
        .save_members("room-2", &sample_members("room-2"))
        .await
        .expect("save members");

    // members.json exists and meta.json never contains the member list.
    let meta_bytes = std::fs::read(store.group_chats_root().join("room-2").join("meta.json"))
        .expect("meta.json exists");
    let meta_text = String::from_utf8_lossy(&meta_bytes);
    assert!(
        !meta_text.contains("member-1"),
        "meta.json must not contain members"
    );

    let members = store.list_members("room-2").await.expect("list members");
    assert_eq!(members.len(), 1);
    assert_eq!(members[0].session_id, "room-2-member-1");
}

#[tokio::test]
async fn group_chat_store_rebuilds_index_when_missing() {
    let root = TestTempDir::new("index-rebuild");
    let store = GroupChatStore::new(root.path().join("group-chats"));

    store
        .save_room(&sample_room("room-a", "Alpha"))
        .await
        .expect("save");
    store
        .save_room(&sample_room("room-b", "Beta"))
        .await
        .expect("save");

    // No index.json written yet (save_room only writes meta.json).
    let index_path = store.group_chats_root().join("index.json");
    assert!(!index_path.exists());

    let (index, rebuilt) = store
        .read_or_rebuild_index()
        .await
        .expect("read or rebuild");
    assert!(rebuilt, "index must be rebuilt when missing");
    assert_eq!(index.rooms.len(), 2);
    assert!(index_path.exists(), "index.json should now exist");

    // Second read is served from cache.
    let (cached, rebuilt_again) = store.read_or_rebuild_index().await.expect("cached");
    assert!(!rebuilt_again);
    assert_eq!(cached.rooms.len(), 2);
}

#[tokio::test]
async fn group_chat_store_update_message_status_resolves_via_catalog() {
    let root = TestTempDir::new("status-update");
    let store = GroupChatStore::new(root.path().join("group-chats"));

    store
        .save_room(&sample_room("room-3", "Status"))
        .await
        .expect("save");
    let message = sample_message("room-3", "msg-status", "pending message");
    store
        .append_message("room-3", &message)
        .await
        .expect("append");

    store
        .update_message_status("room-3", "msg-status", GroupChatMessageStatus::Replied)
        .await
        .expect("update status");

    // Catalog entry carries the id→index mapping and the updated status (P1-2).
    let catalog: StoredGroupChatMessageCatalog = serde_json::from_str(
        &std::fs::read_to_string(
            store
                .group_chats_root()
                .join("room-3")
                .join("message-catalog.json"),
        )
        .expect("catalog exists"),
    )
    .expect("catalog parses");
    assert_eq!(catalog.entries.len(), 1);
    assert_eq!(catalog.entries[0].message_id, "msg-status");
    assert_eq!(catalog.entries[0].index, 0);
    assert_eq!(catalog.entries[0].status, GroupChatMessageStatus::Replied);

    // Message file itself is updated.
    let window = store
        .list_messages("room-3", None, None)
        .await
        .expect("list");
    assert_eq!(window.messages[0].status, GroupChatMessageStatus::Replied);
}

#[tokio::test]
async fn group_chat_store_delete_room_removes_dir_and_index_entry() {
    let root = TestTempDir::new("delete-room");
    let store = GroupChatStore::new(root.path().join("group-chats"));

    store
        .save_room(&sample_room("room-del", "To Delete"))
        .await
        .expect("save");
    store
        .save_members("room-del", &sample_members("room-del"))
        .await
        .expect("save members");
    store
        .append_message("room-del", &sample_message("room-del", "msg-del", "x"))
        .await
        .expect("append");

    store.rebuild_index().await.expect("rebuild index");
    let (index_before, _) = store.read_or_rebuild_index().await.expect("read index");
    assert_eq!(index_before.rooms.len(), 1);

    store.delete_room("room-del").await.expect("delete room");

    let room_dir = store.group_chats_root().join("room-del");
    assert!(!room_dir.exists(), "room directory must be gone");

    let (index_after, _) = store.read_or_rebuild_index().await.expect("read index");
    assert!(index_after.rooms.is_empty(), "index must have no residue");

    let err = store.load_room("room-del").await.expect_err("room gone");
    assert!(matches!(err, GroupChatStoreError::RoomNotFound(_)));
}

#[tokio::test]
async fn group_chat_store_damaged_meta_does_not_take_down_listing() {
    let root = TestTempDir::new("damaged-meta");
    let store = GroupChatStore::new(root.path().join("group-chats"));

    store
        .save_room(&sample_room("room-ok", "Healthy"))
        .await
        .expect("save");
    let damaged_dir = store.group_chats_root().join("room-broken");
    std::fs::create_dir_all(&damaged_dir).expect("damaged dir");
    std::fs::write(damaged_dir.join("meta.json"), "{not valid json").expect("write damaged");

    let (rooms, damaged) = store.list_rooms().await.expect("listing survives damage");
    assert_eq!(rooms.len(), 1);
    assert_eq!(rooms[0].room_id, "room-ok");
    assert_eq!(damaged, vec!["room-broken"]);
}

#[tokio::test]
async fn group_chat_store_damaged_members_degrades_to_empty_without_failing() {
    let root = TestTempDir::new("damaged-members");
    let store = GroupChatStore::new(root.path().join("group-chats"));

    store
        .save_room(&sample_room("room-4", "Members Damage"))
        .await
        .expect("save");
    let room_dir = store.group_chats_root().join("room-4");
    std::fs::write(room_dir.join("members.json"), "garbage").expect("damaged members");

    let members = store
        .list_members("room-4")
        .await
        .expect("members degrade to empty");
    assert!(members.is_empty());

    let loaded = store.load_room("room-4").await.expect("room still loads");
    assert_eq!(loaded.room_id, "room-4");
    assert!(loaded.members.is_empty());
}

#[tokio::test]
async fn group_chat_store_list_messages_paginates_with_cursor() {
    let root = TestTempDir::new("paginate");
    let store = GroupChatStore::new(root.path().join("group-chats"));

    store
        .save_room(&sample_room("room-5", "Pages"))
        .await
        .expect("save");
    for i in 0..5 {
        let message = sample_message("room-5", &format!("msg-{i}"), &format!("content-{i}"));
        store
            .append_message("room-5", &message)
            .await
            .expect("append");
    }

    let page1 = store
        .list_messages("room-5", Some(2), None)
        .await
        .expect("page 1");
    assert_eq!(page1.messages.len(), 2);
    assert_eq!(page1.messages[0].message_id, "msg-3");
    assert_eq!(page1.messages[1].message_id, "msg-4");
    assert_eq!(page1.next_cursor, Some(3));

    let page2 = store
        .list_messages("room-5", Some(2), page1.next_cursor)
        .await
        .expect("page 2");
    assert_eq!(page2.messages.len(), 2);
    assert_eq!(page2.messages[0].message_id, "msg-1");
    assert_eq!(page2.messages[1].message_id, "msg-2");
    assert_eq!(page2.next_cursor, Some(1));

    let page3 = store
        .list_messages("room-5", Some(2), page2.next_cursor)
        .await
        .expect("page 3");
    assert_eq!(page3.messages.len(), 1);
    assert_eq!(page3.messages[0].message_id, "msg-0");
    assert_eq!(page3.next_cursor, None);
}

#[tokio::test]
async fn group_chat_store_room_lock_serializes_concurrent_writes() {
    let root = TestTempDir::new("concurrent");
    let store = std::sync::Arc::new(GroupChatStore::new(root.path().join("group-chats")));
    store
        .save_room(&sample_room("room-6", "Concurrent"))
        .await
        .expect("save");

    let mut handles = Vec::new();
    for i in 0..20 {
        let store = store.clone();
        handles.push(tokio::spawn(async move {
            let message = sample_message("room-6", &format!("msg-c-{i}"), "hi");
            store
                .append_message("room-6", &message)
                .await
                .expect("append");
        }));
    }
    for handle in handles {
        handle.await.expect("join");
    }

    let window = store
        .list_messages("room-6", None, None)
        .await
        .expect("list");
    assert_eq!(window.messages.len(), 20, "no message may be lost");

    // All message files are sequentially indexed 0..19 with no gaps.
    let catalog: StoredGroupChatMessageCatalog = serde_json::from_str(
        &std::fs::read_to_string(
            store
                .group_chats_root()
                .join("room-6")
                .join("message-catalog.json"),
        )
        .expect("catalog exists"),
    )
    .expect("catalog parses");
    let mut indexes: Vec<usize> = catalog.entries.iter().map(|entry| entry.index).collect();
    indexes.sort_unstable();
    assert_eq!(indexes, (0..20).collect::<Vec<_>>());
}

#[tokio::test]
async fn group_chat_store_rejects_invalid_room_ids() {
    let root = TestTempDir::new("invalid-room");
    let store = GroupChatStore::new(root.path().join("group-chats"));

    assert!(matches!(
        store.load_room("../escape").await,
        Err(GroupChatStoreError::InvalidRoomId(_))
    ));
    assert!(matches!(
        store.list_members("a/b").await,
        Err(GroupChatStoreError::InvalidRoomId(_))
    ));
}

#[tokio::test]
async fn group_chat_store_scan_timed_out_messages_marks_old_pending_and_delivered() {
    let root = TestTempDir::new("timeout-scan");
    let store = GroupChatStore::new(root.path().join("group-chats"));

    store
        .save_room(&sample_room("room-7", "Timeout"))
        .await
        .expect("save");

    // A message older than the timeout still awaiting a reply.
    let mut old_pending = sample_message("room-7", "msg-old-pending", "old pending");
    old_pending.timestamp = 1_000;
    store
        .append_message("room-7", &old_pending)
        .await
        .expect("append old pending");

    // A message older than the timeout already delivered (still awaiting reply).
    let mut old_delivered = sample_message("room-7", "msg-old-delivered", "old delivered");
    old_delivered.timestamp = 1_000;
    old_delivered.status = GroupChatMessageStatus::Delivered;
    store
        .append_message("room-7", &old_delivered)
        .await
        .expect("append old delivered");

    // A fresh message must NOT be touched.
    let mut fresh = sample_message("room-7", "msg-fresh", "fresh");
    fresh.timestamp = 900_000;
    store
        .append_message("room-7", &fresh)
        .await
        .expect("append fresh");

    // A replied message older than the timeout must NOT be touched.
    let mut replied = sample_message("room-7", "msg-replied", "replied");
    replied.timestamp = 1_000;
    replied.status = GroupChatMessageStatus::Replied;
    store
        .append_message("room-7", &replied)
        .await
        .expect("append replied");

    // now = 1_000_000; timeout 300s → cutoff 700_000. Old messages (1_000)
    // exceed the cutoff; fresh (900_000) and replied do not.
    let timed_out = store
        .scan_timed_out_messages("room-7", 300, 1_000_000)
        .await
        .expect("scan");

    let mut ids: Vec<&str> = timed_out.iter().map(|m| m.message_id.as_str()).collect();
    ids.sort_unstable();
    assert_eq!(ids, vec!["msg-old-delivered", "msg-old-pending"]);
    assert!(
        timed_out
            .iter()
            .all(|m| m.status == GroupChatMessageStatus::Failed),
        "timed-out messages must be marked failed"
    );

    // The message files themselves are updated to Failed.
    let window = store
        .list_messages("room-7", None, None)
        .await
        .expect("list");
    let by_id: std::collections::HashMap<_, _> = window
        .messages
        .iter()
        .map(|m| (m.message_id.as_str(), m.status))
        .collect();
    assert_eq!(by_id["msg-old-pending"], GroupChatMessageStatus::Failed);
    assert_eq!(by_id["msg-old-delivered"], GroupChatMessageStatus::Failed);
    assert_eq!(by_id["msg-fresh"], GroupChatMessageStatus::Pending);
    assert_eq!(by_id["msg-replied"], GroupChatMessageStatus::Replied);
}

#[tokio::test]
async fn group_chat_store_scan_timed_out_messages_zero_timeout_is_noop() {
    let root = TestTempDir::new("timeout-zero");
    let store = GroupChatStore::new(root.path().join("group-chats"));

    store
        .save_room(&sample_room("room-8", "Zero"))
        .await
        .expect("save");
    let mut old = sample_message("room-8", "msg-0", "old");
    old.timestamp = 1_000;
    store.append_message("room-8", &old).await.expect("append");

    // reply_timeout_secs = 0 disables the scan entirely.
    let timed_out = store
        .scan_timed_out_messages("room-8", 0, 1_000_000)
        .await
        .expect("scan disabled");
    assert!(timed_out.is_empty());

    let window = store
        .list_messages("room-8", None, None)
        .await
        .expect("list");
    assert_eq!(window.messages[0].status, GroupChatMessageStatus::Pending);
}

#[tokio::test]
async fn group_chat_store_delete_room_clears_all_message_files_explicitly() {
    // R-GC-23 边界 7 强化：防幽灵——删除后消息文件显式全清（不仅目录消失）。
    let root = TestTempDir::new("delete-messages-explicit");
    let store = GroupChatStore::new(root.path().join("group-chats"));

    store
        .save_room(&sample_room("room-del-msg", "Del Msg"))
        .await
        .expect("save");
    for i in 0..3 {
        store
            .append_message(
                "room-del-msg",
                &sample_message("room-del-msg", &format!("msg-{i}"), &format!("content-{i}")),
            )
            .await
            .expect("append");
    }

    let messages_dir = store
        .group_chats_root()
        .join("room-del-msg")
        .join("messages");
    assert!(messages_dir.exists());
    assert_eq!(std::fs::read_dir(&messages_dir).expect("read").count(), 3);

    store.delete_room("room-del-msg").await.expect("delete");

    assert!(
        !store.group_chats_root().join("room-del-msg").exists(),
        "room dir must be gone (covers messages/)"
    );

    // index 重建后无残留（R-GC-04 delete_room 契约）。
    let (index, _) = store.read_or_rebuild_index().await.expect("index");
    assert!(
        index
            .rooms
            .iter()
            .all(|room| room.room_id != "room-del-msg"),
        "index must not reference the deleted room"
    );
}

#[tokio::test]
async fn group_chat_store_leave_semantics_excludes_removed_member() {
    // R-GC-23 边界 3：成员退出后不再出现在成员列表（消息广播目标由成员列表决定，
    // 退出者自然不再收消息）。
    let root = TestTempDir::new("leave-exclude");
    let store = GroupChatStore::new(root.path().join("group-chats"));

    store
        .save_room(&sample_room("room-leave", "Leave"))
        .await
        .expect("save");
    let members = vec![
        GroupChatMember {
            session_id: "m-1".to_string(),
            role: GroupChatMemberRole::Member,
            joined_at: 1,
            agent_type: "Claw".to_string(),
            display_name: None,
        },
        GroupChatMember {
            session_id: "m-2".to_string(),
            role: GroupChatMemberRole::Member,
            joined_at: 1,
            agent_type: "Claw".to_string(),
            display_name: None,
        },
    ];
    store
        .save_members("room-leave", &members)
        .await
        .expect("save members");

    // m-1 退出：成员列表只剩 m-2。
    let remaining: Vec<GroupChatMember> = members
        .into_iter()
        .filter(|member| member.session_id != "m-1")
        .collect();
    store
        .save_members("room-leave", &remaining)
        .await
        .expect("save remaining");

    let after = store.list_members("room-leave").await.expect("list");
    assert_eq!(after.len(), 1);
    assert_eq!(
        after[0].session_id, "m-2",
        "退出者 m-1 不再收消息（不在成员列表）"
    );
}
