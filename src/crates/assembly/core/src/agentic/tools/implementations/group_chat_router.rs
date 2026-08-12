//! GroupChatRouter — message routing (A free broadcast / B round-robin /
//! @all) plus reply ingestion (回执收集, P1-5).
//!
//! Contract: type-contract v1.3 §1.3 + dispatch-prompts v1.3 R-GC-10.
//!
//! Routing decisions:
//! - Free mode: broadcast to every member (each with its own reply_route).
//! - RoundRobin mode: `RoundRobin::next` picks one member; the cursor is
//!   persisted back through the room record (P1-10).
//! - @all (`[{kind:"all"}]`, P1-4): explicit all-member dispatch with urgent
//!   semantics.
//!
//! Reply collection: a member's reply carries `groupId`/`groupMessageId`
//! metadata (R-GC-11); the reply arrives back through the agent-session reply
//! route with those keys preserved (scheduler strips only `sender*`). The
//! router's `ingest_reply` entry resolves the original message through the
//! catalog and updates its status to `Replied` (P1-5/P1-2). Timeout scanning
//! stays in R-GC-26 (this router never scans timeouts).

use super::group_chat_tool::store_tool_error;
use crate::agentic::coordination::{round_robin_next, ConversationCoordinator};
use crate::service_agent_runtime::CoreServiceAgentRuntime;
use crate::util::errors::{BitFunError, BitFunResult};
use bitfun_runtime_ports::{
    AgentDialogTurnRequest, DialogSubmissionPolicy, DialogTriggerSource, GroupChatActor,
    GroupChatMessageStatus, GroupChatMode,
};
use bitfun_services_core::session::{GroupChatStore, GroupChatStoreError};
use serde_json::{json, Map as JsonMap, Value};
use std::sync::Arc;

/// Resolved dispatch targets for one group message.
#[derive(Debug, Clone)]
pub(crate) struct GroupChatDispatchPlan {
    /// Target member session ids (deduplicated).
    pub targets: Vec<String>,
    /// True when the dispatch was an explicit @all (P1-4).
    /// 保留为契约语义标记（P1-4 显式 @全体）；send 结果面消费时读取。
    #[allow(dead_code)]
    pub mention_all: bool,
    /// True when urgent semantics apply (interrupt priority).
    pub urgent: bool,
}

/// The group-chat routing layer (R-GC-10).
pub(crate) struct GroupChatRouter;

/// Deterministic uuid-like id from a seed (mirrors group_chat_tool.rs so the
/// message ids are stable within one room/message/timestamp combination).
fn uuid_v4_deterministic(seed: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(b"bitfun-group-chat-v1\0");
    hasher.update(seed.as_bytes());
    let digest = hasher.finalize();
    let hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    hex.chars().take(32).collect()
}

impl GroupChatRouter {
    /// Resolves dispatch targets by mode + mention targets:
    /// - Free + empty mentions → all members.
    /// - RoundRobin + empty mentions → one member by cursor (cursor advanced).
    /// - explicit `[{kind:"all"}]` → all members with urgent semantics (P1-4).
    /// - `[{kind:"claw",...}]` → the targeted members only.
    ///
    /// Returns `None` when the resolved target list is empty (caller maps to
    /// EmptyMembers / no-target error).
    pub(crate) async fn resolve_dispatch_plan(
        store: &GroupChatStore,
        room: &bitfun_runtime_ports::GroupChatRoom,
        mention_targets: &[GroupChatActor],
        urgent: bool,
    ) -> Result<GroupChatDispatchPlan, BitFunError> {
        let mention_all = mention_targets
            .iter()
            .any(|target| matches!(target, GroupChatActor::All));
        let targeted_ids: Vec<String> = mention_targets
            .iter()
            .filter_map(|target| match target {
                GroupChatActor::Claw { session_id, .. } => Some(session_id.clone()),
                _ => None,
            })
            .collect();

        if mention_all {
            // P1-4: explicit @all → all members + urgent semantics.
            let targets: Vec<String> = room
                .members
                .iter()
                .map(|member| member.session_id.clone())
                .collect();
            return Ok(GroupChatDispatchPlan {
                targets,
                mention_all: true,
                urgent: true,
            });
        }
        if !targeted_ids.is_empty() {
            return Ok(GroupChatDispatchPlan {
                targets: targeted_ids,
                mention_all: false,
                urgent,
            });
        }
        match room.mode {
            GroupChatMode::Free => {
                let targets: Vec<String> = room
                    .members
                    .iter()
                    .map(|member| member.session_id.clone())
                    .collect();
                Ok(GroupChatDispatchPlan {
                    targets,
                    mention_all: false,
                    urgent,
                })
            }
            GroupChatMode::RoundRobin => {
                let members: Vec<String> = room
                    .members
                    .iter()
                    .map(|member| member.session_id.clone())
                    .collect();
                if members.is_empty() {
                    return Ok(GroupChatDispatchPlan {
                        targets: Vec::new(),
                        mention_all: false,
                        urgent,
                    });
                }
                let picked = round_robin_next(&members, room.round_robin_cursor);
                let Some(picked) = picked else {
                    return Ok(GroupChatDispatchPlan {
                        targets: Vec::new(),
                        mention_all: false,
                        urgent,
                    });
                };
                // Advance the cursor and persist it (P1-10: cursor 后端落盘).
                let next_cursor = (room.round_robin_cursor + 1) % members.len();
                let mut updated = room.clone();
                updated.round_robin_cursor = next_cursor;
                store.save_room(&updated).await.map_err(store_tool_error)?;
                Ok(GroupChatDispatchPlan {
                    targets: vec![picked],
                    mention_all: false,
                    urgent,
                })
            }
        }
    }

    /// Ingest one member reply (P1-5): resolves the original group message via
    /// `groupId` + `groupMessageId` metadata, marks it `Replied` (P1-2), and
    /// appends the reply content as a new Agent message in the room (P2-1).
    ///
    /// Returns `Ok(())` when the correlation keys are absent (a non-group reply
    /// is not a group-chat event and is ignored here — the reply still reaches
    /// the initiating session through the normal reply route).
    /// 契约 §1.3 端口方法 ingest_reply 的 router 实现；reply 路由接线消费。
    pub(crate) async fn ingest_reply(
        store: &GroupChatStore,
        metadata: &JsonMap<String, Value>,
        reply_content: &str,
        reply_author: &GroupChatActor,
        timestamp: i64,
    ) -> BitFunResult<()> {
        let Some(room_id) = metadata.get("groupId").and_then(Value::as_str) else {
            return Ok(());
        };
        let Some(message_id) = metadata.get("groupMessageId").and_then(Value::as_str) else {
            return Ok(());
        };
        // P2-2: a late reply to an already-deleted message (or room) is not an
        // error — the room/message is gone, so the ingest is a no-op instead of
        // bubbling MessageNotFound back into the reply forwarding path.
        match store
            .update_message_status(room_id, message_id, GroupChatMessageStatus::Replied)
            .await
        {
            Ok(()) => {}
            Err(error) if matches!(error, GroupChatStoreError::MessageNotFound(_)) => {
                return Ok(());
            }
            Err(error) => return Err(store_tool_error(error)),
        }
        // P2-1: persist the reply body into the group stream so the room shows
        // the reply text, not just the Replied badge.
        if !reply_content.trim().is_empty() {
            let reply = bitfun_runtime_ports::GroupChatMessage {
                message_id: format!(
                    "msg-reply-{}",
                    uuid_v4_deterministic(&format!(
                        "{room_id}-{message_id}-{timestamp}-{reply_content}"
                    ))
                ),
                room_id: room_id.to_string(),
                author: reply_author.clone(),
                kind: bitfun_runtime_ports::GroupChatMessageKind::Agent,
                content: reply_content.to_string(),
                mention_targets: Vec::new(),
                reply_to_message_id: Some(message_id.to_string()),
                timestamp,
                status: GroupChatMessageStatus::Delivered,
            };
            store
                .append_message(room_id, &reply)
                .await
                .map_err(store_tool_error)?;
        }
        Ok(())
    }

    /// Constructs one per-member dialog-turn request carrying the group
    /// correlation metadata (R-GC-11) and, for RoundRobin/@all, urgent policy.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn build_dispatch_request(
        coordinator: &Arc<ConversationCoordinator>,
        workspace_path: &str,
        room_id: &str,
        message_id: &str,
        content: &str,
        target_session_id: &str,
        group_author: &str,
        urgent: bool,
    ) -> BitFunResult<AgentDialogTurnRequest> {
        let session_manager = coordinator.get_session_manager();
        let session = session_manager
            .get_session(target_session_id)
            .ok_or_else(|| {
                BitFunError::tool(format!(
                    "group chat dispatch target session '{target_session_id}' does not exist"
                ))
            })?;

        let mut metadata = JsonMap::new();
        metadata.insert("groupId".to_string(), json!(room_id));
        metadata.insert("groupMessageId".to_string(), json!(message_id));
        metadata.insert("groupAuthor".to_string(), json!(group_author));

        let policy = DialogSubmissionPolicy::for_source(DialogTriggerSource::AgentSession)
            .with_queue_priority(if urgent {
                bitfun_runtime_ports::DialogQueuePriority::High
            } else {
                bitfun_runtime_ports::DialogQueuePriority::Low
            });

        // P0-3: a Claw-initiated group message carries a real reply route back
        // to the initiating member (the reply is forwarded to that session AND
        // ingested into the room via groupId/groupMessageId metadata). The
        // master has no session id, so its route stays None — the reply is
        // still ingested by the group-chat hook in process_turn_outcome.
        let reply_route = if group_author == bitfun_runtime_ports::GROUP_MASTER_ACTOR {
            None
        } else {
            Some(bitfun_runtime_ports::AgentSessionReplyRoute {
                source_session_id: group_author.to_string(),
                source_workspace_path: workspace_path.to_string(),
                source_remote_connection_id: None,
                source_remote_ssh_host: None,
            })
        };

        Ok(AgentDialogTurnRequest {
            session_id: target_session_id.to_string(),
            message: content.to_string(),
            original_message: None,
            turn_id: None,
            execution: Default::default(),
            agent_type: session.agent_type,
            workspace_path: Some(workspace_path.to_string()),
            remote_connection_id: None,
            remote_ssh_host: None,
            policy,
            reply_route,
            prepended_reminders: Vec::new(),
            attachments: Vec::new(),
            metadata,
        })
    }

    /// Dispatches one message to `targets` via the agent runtime.
    /// Returns (delivered, failed) session id lists.
    /// 8 个参数均为派发上下文原子字段（契约 R-GC-08/10），保持平铺可读。
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn dispatch_to_targets(
        coordinator: &Arc<ConversationCoordinator>,
        workspace_path: &str,
        room_id: &str,
        message_id: &str,
        content: &str,
        group_author: &str,
        urgent: bool,
        targets: &[String],
    ) -> (Vec<String>, Vec<Value>) {
        let scheduler = match crate::agentic::coordination::get_global_scheduler() {
            Some(scheduler) => scheduler,
            None => {
                let failed: Vec<Value> = targets
                    .iter()
                    .map(|id| json!({ "sessionId": id, "reason": "scheduler not initialized" }))
                    .collect();
                return (Vec::new(), failed);
            }
        };
        let runtime = match CoreServiceAgentRuntime::agent_runtime_with_dialog_turns(
            coordinator.clone(),
            scheduler.clone(),
        ) {
            Ok(runtime) => runtime,
            Err(error) => {
                let failed: Vec<Value> = targets
                    .iter()
                    .map(|id| json!({ "sessionId": id, "reason": error.to_string() }))
                    .collect();
                return (Vec::new(), failed);
            }
        };

        let mut delivered = Vec::new();
        let mut failed = Vec::new();
        for target in targets {
            let request = match Self::build_dispatch_request(
                coordinator,
                workspace_path,
                room_id,
                message_id,
                content,
                target,
                group_author,
                urgent,
            ) {
                Ok(request) => request,
                Err(error) => {
                    failed.push(json!({ "sessionId": target, "reason": error.to_string() }));
                    continue;
                }
            };
            match runtime.submit_dialog_turn(request).await {
                Ok(_) => delivered.push(target.clone()),
                Err(error) => {
                    failed.push(json!({ "sessionId": target, "reason": error.to_string() }));
                }
            }
        }
        (delivered, failed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitfun_runtime_ports::{
        GroupChatMember, GroupChatMemberRole, GroupChatMessage, GroupChatRoom, GroupChatStatus,
    };
    use bitfun_services_core::session::GroupChatStore;
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
            let path =
                std::env::temp_dir().join(format!("bitfun-group-chat-router-{name}-{nonce}"));
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

    fn members(names: &[&str]) -> Vec<GroupChatMember> {
        names
            .iter()
            .enumerate()
            .map(|(index, name)| GroupChatMember {
                session_id: name.to_string(),
                role: if index == 0 {
                    GroupChatMemberRole::Owner
                } else {
                    GroupChatMemberRole::Member
                },
                joined_at: 1,
                agent_type: "Claw".to_string(),
                display_name: None,
            })
            .collect()
    }

    fn room(room_id: &str, mode: GroupChatMode, cursor: usize) -> GroupChatRoom {
        GroupChatRoom {
            schema_version: 1,
            room_id: room_id.to_string(),
            name: room_id.to_string(),
            owner: GroupChatActor::Master,
            mode,
            round_robin_cursor: cursor,
            created_at: 1,
            last_active_at: 1,
            status: GroupChatStatus::Active,
            member_limit: 50,
            members: members(&["m-1", "m-2", "m-3"]),
        }
    }

    #[tokio::test]
    async fn free_mode_empty_mentions_broadcasts_to_all_members() {
        let root = TestTempDir::new("free");
        let store = GroupChatStore::new(root.path().join("group-chats"));
        let room = room("room-1", GroupChatMode::Free, 0);

        let plan = GroupChatRouter::resolve_dispatch_plan(&store, &room, &[], false)
            .await
            .expect("plan");
        assert_eq!(plan.targets, vec!["m-1", "m-2", "m-3"]);
        assert!(!plan.mention_all);
        assert!(!plan.urgent);
    }

    #[tokio::test]
    async fn round_robin_picks_one_member_and_persists_cursor() {
        let root = TestTempDir::new("rr");
        let store = GroupChatStore::new(root.path().join("group-chats"));
        let room = room("room-rr", GroupChatMode::RoundRobin, 0);
        // Persist members so load_room can read them back (P1-11 single source).
        store
            .save_members("room-rr", &room.members)
            .await
            .expect("save members");

        // First dispatch → m-1; cursor advances to 1 and is persisted.
        let plan = GroupChatRouter::resolve_dispatch_plan(&store, &room, &[], false)
            .await
            .expect("plan");
        assert_eq!(plan.targets, vec!["m-1"]);

        let reloaded = store.load_room("room-rr").await.expect("reload");
        assert_eq!(reloaded.round_robin_cursor, 1);
        assert_eq!(reloaded.members.len(), 3);

        // Second dispatch on the reloaded room → m-2; cursor → 2.
        let plan2 = GroupChatRouter::resolve_dispatch_plan(&store, &reloaded, &[], false)
            .await
            .expect("plan2");
        assert_eq!(plan2.targets, vec!["m-2"]);
        let reloaded2 = store.load_room("room-rr").await.expect("reload2");
        assert_eq!(reloaded2.round_robin_cursor, 2);

        // Third → m-3; cursor wraps to 0 (mod 3).
        let plan3 = GroupChatRouter::resolve_dispatch_plan(&store, &reloaded2, &[], false)
            .await
            .expect("plan3");
        assert_eq!(plan3.targets, vec!["m-3"]);
        let reloaded3 = store.load_room("room-rr").await.expect("reload3");
        assert_eq!(reloaded3.round_robin_cursor, 0);
    }

    #[tokio::test]
    async fn mention_all_is_explicit_full_broadcast_with_urgent() {
        let root = TestTempDir::new("all");
        let store = GroupChatStore::new(root.path().join("group-chats"));
        let room = room("room-all", GroupChatMode::RoundRobin, 0);

        let plan =
            GroupChatRouter::resolve_dispatch_plan(&store, &room, &[GroupChatActor::All], false)
                .await
                .expect("plan");
        assert_eq!(plan.targets, vec!["m-1", "m-2", "m-3"]);
        assert!(plan.mention_all);
        assert!(plan.urgent, "@all carries urgent semantics (P1-4)");
    }

    #[tokio::test]
    async fn targeted_mentions_dispatch_only_specified_members() {
        let root = TestTempDir::new("targeted");
        let store = GroupChatStore::new(root.path().join("group-chats"));
        let room = room("room-t", GroupChatMode::Free, 0);

        let plan = GroupChatRouter::resolve_dispatch_plan(
            &store,
            &room,
            &[GroupChatActor::Claw {
                session_id: "m-2".to_string(),
                agent_type: "Claw".to_string(),
            }],
            true,
        )
        .await
        .expect("plan");
        assert_eq!(plan.targets, vec!["m-2"]);
        assert!(!plan.mention_all);
        assert!(plan.urgent);
    }

    #[tokio::test]
    async fn empty_members_resolves_to_empty_targets_without_error() {
        let root = TestTempDir::new("empty");
        let store = GroupChatStore::new(root.path().join("group-chats"));
        let mut room = room("room-e", GroupChatMode::Free, 0);
        room.members = Vec::new();

        let plan = GroupChatRouter::resolve_dispatch_plan(&store, &room, &[], false)
            .await
            .expect("plan");
        assert!(plan.targets.is_empty());
    }

    #[test]
    fn ingest_reply_ignores_metadata_without_group_keys() {
        let empty = JsonMap::new();
        // Returns Ok(()) without touching any store — the reply is a normal
        // agent-session reply, not a group-chat event.
        assert!(futures::executor::block_on(GroupChatRouter::ingest_reply(
            &GroupChatStore::new(PathBuf::from("unused")),
            &empty,
            "reply",
            &GroupChatActor::Claw {
                session_id: "m-1".to_string(),
                agent_type: "Claw".to_string(),
            },
            1,
        ))
        .is_ok());
    }

    #[tokio::test]
    async fn ingest_reply_marks_message_replied_and_persists_reply_body() {
        let root = TestTempDir::new("ingest");
        let store = GroupChatStore::new(root.path().join("group-chats"));
        store
            .save_room(&room("room-i", GroupChatMode::Free, 0))
            .await
            .expect("save room");

        // A message in Pending state (awaiting member replies).
        let message = GroupChatMessage {
            message_id: "msg-1".to_string(),
            room_id: "room-i".to_string(),
            author: GroupChatActor::Master,
            kind: bitfun_runtime_ports::GroupChatMessageKind::User,
            content: "question".to_string(),
            mention_targets: Vec::new(),
            reply_to_message_id: None,
            timestamp: 1,
            status: GroupChatMessageStatus::Pending,
        };
        store
            .append_message("room-i", &message)
            .await
            .expect("append");

        // A member reply arrives with the correlation keys (R-GC-11 metadata).
        let mut metadata = JsonMap::new();
        metadata.insert("groupId".to_string(), json!("room-i"));
        metadata.insert("groupMessageId".to_string(), json!("msg-1"));
        metadata.insert("groupAuthor".to_string(), json!("m-1"));
        GroupChatRouter::ingest_reply(
            &store,
            &metadata,
            "here is my answer",
            &GroupChatActor::Claw {
                session_id: "m-1".to_string(),
                agent_type: "Claw".to_string(),
            },
            42,
        )
        .await
        .expect("ingest");

        // The original message is now Replied (P1-5/P1-2) and the reply body is
        // appended to the room stream (P2-1).
        let window = store
            .list_messages("room-i", None, None)
            .await
            .expect("list");
        assert_eq!(window.messages.len(), 2);
        assert_eq!(window.messages[0].status, GroupChatMessageStatus::Replied);
        assert_eq!(window.messages[1].content, "here is my answer");
        assert_eq!(
            window.messages[1].reply_to_message_id.as_deref(),
            Some("msg-1")
        );
        assert_eq!(window.messages[1].status, GroupChatMessageStatus::Delivered);
    }

    #[tokio::test]
    async fn ingest_reply_skips_body_when_empty() {
        let root = TestTempDir::new("ingest-empty");
        let store = GroupChatStore::new(root.path().join("group-chats"));
        store
            .save_room(&room("room-e2", GroupChatMode::Free, 0))
            .await
            .expect("save room");
        let message = GroupChatMessage {
            message_id: "msg-1".to_string(),
            room_id: "room-e2".to_string(),
            author: GroupChatActor::Master,
            kind: bitfun_runtime_ports::GroupChatMessageKind::User,
            content: "question".to_string(),
            mention_targets: Vec::new(),
            reply_to_message_id: None,
            timestamp: 1,
            status: GroupChatMessageStatus::Pending,
        };
        store
            .append_message("room-e2", &message)
            .await
            .expect("append");

        let mut metadata = JsonMap::new();
        metadata.insert("groupId".to_string(), json!("room-e2"));
        metadata.insert("groupMessageId".to_string(), json!("msg-1"));
        GroupChatRouter::ingest_reply(
            &store,
            &metadata,
            "   ",
            &GroupChatActor::Claw {
                session_id: "m-1".to_string(),
                agent_type: "Claw".to_string(),
            },
            42,
        )
        .await
        .expect("ingest");

        let window = store
            .list_messages("room-e2", None, None)
            .await
            .expect("list");
        assert_eq!(window.messages.len(), 1, "empty body appends nothing");
        assert_eq!(window.messages[0].status, GroupChatMessageStatus::Replied);
    }

    #[tokio::test]
    async fn single_member_room_broadcast_resolves_to_that_member() {
        // R-GC-23 边界 2：单成员群 → 广播等价定向（空 mention 全员 = 1 目标）。
        let root = TestTempDir::new("single-member");
        let store = GroupChatStore::new(root.path().join("group-chats"));
        let mut room = room("room-s", GroupChatMode::Free, 0);
        room.members = vec![GroupChatMember {
            session_id: "only-m".to_string(),
            role: GroupChatMemberRole::Owner,
            joined_at: 1,
            agent_type: "Claw".to_string(),
            display_name: None,
        }];

        let plan = GroupChatRouter::resolve_dispatch_plan(&store, &room, &[], false)
            .await
            .expect("plan");
        assert_eq!(plan.targets, vec!["only-m"], "单成员广播 = 仅该成员");
    }
}
