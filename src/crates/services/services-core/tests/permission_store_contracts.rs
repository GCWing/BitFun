#![cfg(feature = "permission")]

use bitfun_runtime_ports::{
    PermissionAuditEvent, PermissionAuditRecord, PermissionAuditStorePort, PermissionGrant,
    PermissionGrantKey, PermissionGrantStorePort, PermissionReply, PermissionReplySource,
    PermissionReplyStorePort, PermissionRequest, PermissionRequestSource,
    PermissionRequestSourceKind,
};
use bitfun_services_core::permission_store::ProjectPermissionSqliteStore;
use serde_json::Map;

fn request(request_id: &str, project_id: &str) -> PermissionRequest {
    PermissionRequest {
        request_id: request_id.to_string(),
        round_id: format!("synthetic:{request_id}"),
        order: 0,
        tool_call_id: None,
        project_path: None,
        project_id: project_id.to_string(),
        session_id: "session-1".to_string(),
        agent_id: "agentic".to_string(),
        action: "read".to_string(),
        resources: vec!["README.md".to_string()],
        save_resources: vec!["README.md".to_string()],
        source: PermissionRequestSource {
            kind: PermissionRequestSourceKind::ToolCall,
            identity: "tool-1".to_string(),
        },
        delegation: None,
        display_metadata: Map::new(),
    }
}

fn audit_record(
    audit_id: &str,
    request_id: &str,
    project_id: &str,
    timestamp_ms: i64,
) -> PermissionAuditRecord {
    PermissionAuditRecord {
        audit_id: audit_id.to_string(),
        request: request(request_id, project_id),
        event: PermissionAuditEvent::Requested,
        timestamp_ms,
    }
}

#[tokio::test]
async fn project_grants_are_idempotent_isolated_and_survive_store_recreation() {
    let root = tempfile::tempdir().expect("temp permission store");
    let store = ProjectPermissionSqliteStore::new(root.path());
    assert_eq!(
        store.path(),
        root.path().join("tool-permissions.sqlite").as_path()
    );
    let project_a = PermissionGrant {
        project_id: "project-a".to_string(),
        action: "read".to_string(),
        resource: "README.md".to_string(),
        created_at_ms: 10,
    };
    let project_b = PermissionGrant {
        project_id: "project-b".to_string(),
        action: "edit".to_string(),
        resource: "src/*".to_string(),
        created_at_ms: 20,
    };

    store
        .add_project_grants(vec![project_a.clone(), project_a.clone(), project_b])
        .await
        .expect("persist grants");
    assert!(store.path().is_file());
    let connection = rusqlite::Connection::open(store.path()).expect("open permission database");
    let schema_version = connection
        .query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
        .expect("read permission database schema version");
    assert_eq!(schema_version, 1);

    let reopened = ProjectPermissionSqliteStore::new(root.path());
    assert_eq!(
        reopened
            .list_project_grants("project-a")
            .await
            .expect("list project grants"),
        vec![project_a.clone()]
    );
    assert_eq!(
        reopened
            .list_project_grants("project-b")
            .await
            .expect("list other project grants")
            .len(),
        1
    );

    assert!(reopened
        .remove_project_grant(PermissionGrantKey {
            project_id: "project-a".to_string(),
            action: "read".to_string(),
            resource: "README.md".to_string(),
        })
        .await
        .expect("remove grant"));
    assert!(reopened
        .list_project_grants("project-a")
        .await
        .expect("list removed project grants")
        .is_empty());
}

#[tokio::test]
async fn clearing_grants_only_removes_the_selected_project() {
    let root = tempfile::tempdir().expect("temp permission store");
    let store = ProjectPermissionSqliteStore::new(root.path());
    store
        .add_project_grants(vec![
            PermissionGrant {
                project_id: "project-a".to_string(),
                action: "read".to_string(),
                resource: "README.md".to_string(),
                created_at_ms: 10,
            },
            PermissionGrant {
                project_id: "project-b".to_string(),
                action: "edit".to_string(),
                resource: "src/*".to_string(),
                created_at_ms: 20,
            },
        ])
        .await
        .expect("persist grants");

    assert_eq!(
        store
            .clear_project_grants("project-a")
            .await
            .expect("clear project grants"),
        1
    );
    assert!(store
        .list_project_grants("project-a")
        .await
        .expect("list cleared project grants")
        .is_empty());
    assert_eq!(
        store
            .list_project_grants("project-b")
            .await
            .expect("list retained project grants")
            .len(),
        1
    );
}

#[tokio::test]
async fn audit_records_are_idempotent_project_scoped_and_persistent() {
    let root = tempfile::tempdir().expect("temp permission store");
    let store = ProjectPermissionSqliteStore::new(root.path());
    let record = PermissionAuditRecord {
        audit_id: "request-1:replied".to_string(),
        request: request("request-1", "project-a"),
        event: PermissionAuditEvent::Replied {
            reply: PermissionReply::Once,
            source: PermissionReplySource::User,
        },
        timestamp_ms: 100,
    };

    store
        .append_permission_audit(record.clone())
        .await
        .expect("append audit");
    store
        .append_permission_audit(record.clone())
        .await
        .expect("repeat audit idempotently");
    store
        .append_permission_audit(PermissionAuditRecord {
            timestamp_ms: 101,
            ..record.clone()
        })
        .await
        .expect("repeat audit after retry timestamp change");
    store
        .append_permission_audit(PermissionAuditRecord {
            audit_id: "request-2:requested".to_string(),
            request: request("request-2", "project-b"),
            event: PermissionAuditEvent::Requested,
            timestamp_ms: 90,
        })
        .await
        .expect("append other project audit");

    let reopened = ProjectPermissionSqliteStore::new(root.path());
    assert_eq!(
        reopened
            .list_project_permission_audit("project-a")
            .await
            .expect("list project audit"),
        vec![record]
    );
    assert_eq!(
        reopened
            .list_project_permission_audit("project-b")
            .await
            .expect("list other project audit")
            .len(),
        1
    );
}

#[tokio::test]
async fn reply_transaction_persists_grants_and_audit_in_one_state_update() {
    let root = tempfile::tempdir().expect("temp permission store");
    let store = ProjectPermissionSqliteStore::new(root.path());
    let grant = PermissionGrant {
        project_id: "project-a".to_string(),
        action: "read".to_string(),
        resource: "README.md".to_string(),
        created_at_ms: 100,
    };
    let audit = PermissionAuditRecord {
        audit_id: "request-1:replied".to_string(),
        request: request("request-1", "project-a"),
        event: PermissionAuditEvent::Replied {
            reply: PermissionReply::Always,
            source: PermissionReplySource::User,
        },
        timestamp_ms: 100,
    };

    store
        .commit_permission_reply(vec![grant.clone()], vec![audit.clone()])
        .await
        .expect("commit reply transaction");

    let reopened = ProjectPermissionSqliteStore::new(root.path());
    assert_eq!(
        reopened
            .list_project_grants("project-a")
            .await
            .expect("list committed grants"),
        vec![grant]
    );
    assert_eq!(
        reopened
            .list_project_permission_audit("project-a")
            .await
            .expect("list committed audit"),
        vec![audit]
    );
}

#[tokio::test]
async fn audit_retention_keeps_the_newest_records_for_each_project() {
    let root = tempfile::tempdir().expect("temp permission store");
    let store = ProjectPermissionSqliteStore::new(root.path()).with_audit_limit(2);

    store
        .append_permission_audit(audit_record("project-a:1", "request-1", "project-a", 10))
        .await
        .expect("append first project audit");
    store
        .append_permission_audit(audit_record("project-a:2", "request-2", "project-a", 20))
        .await
        .expect("append second project audit");
    store
        .append_permission_audit(audit_record("project-b:1", "request-3", "project-b", 30))
        .await
        .expect("append other project audit");
    store
        .commit_permission_reply(
            Vec::new(),
            vec![audit_record("project-a:3", "request-4", "project-a", 40)],
        )
        .await
        .expect("commit bounded project audit");

    assert_eq!(
        store
            .list_project_permission_audit("project-a")
            .await
            .expect("list retained project audit")
            .into_iter()
            .map(|record| record.audit_id)
            .collect::<Vec<_>>(),
        vec!["project-a:2", "project-a:3"]
    );
    assert_eq!(
        store
            .list_project_permission_audit("project-b")
            .await
            .expect("list isolated project audit")
            .into_iter()
            .map(|record| record.audit_id)
            .collect::<Vec<_>>(),
        vec!["project-b:1"]
    );
}
