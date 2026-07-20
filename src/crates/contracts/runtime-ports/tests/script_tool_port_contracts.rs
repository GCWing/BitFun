use async_trait::async_trait;
use bitfun_runtime_ports::{
    PortResult, ScriptToolDescriptor, ScriptToolExpectedExport, ScriptToolInvokeRequest,
    ScriptToolInvokeResponse, ScriptToolLoadRequest, ScriptToolLoadResponse, ScriptToolRuntime,
    ScriptToolRuntimeAvailability,
};
use serde_json::json;
use std::sync::Mutex;

#[derive(Default)]
struct FakeRuntime {
    operations: Mutex<Vec<String>>,
    loaded: Mutex<Option<String>>,
}

#[async_trait]
impl ScriptToolRuntime for FakeRuntime {
    async fn availability(&self) -> ScriptToolRuntimeAvailability {
        ScriptToolRuntimeAvailability::Available {
            executable: "node".to_string(),
            version: "v-test".to_string(),
        }
    }

    async fn is_loaded(&self, target_id: &str) -> bool {
        self.loaded.lock().unwrap().as_deref() == Some(target_id)
    }

    async fn wait_until_unloaded(&self, _target_id: &str) -> PortResult<()> {
        Ok(())
    }

    async fn load(&self, request: ScriptToolLoadRequest) -> PortResult<ScriptToolLoadResponse> {
        *self.loaded.lock().unwrap() = Some(request.target_id.clone());
        self.operations
            .lock()
            .unwrap()
            .push(format!("load:{}", request.target_id));
        Ok(ScriptToolLoadResponse {
            target_id: request.target_id,
            revision: request.revision,
            tools: vec![ScriptToolDescriptor {
                export_name: "default".to_string(),
                name: "weather".to_string(),
                description: "Weather lookup".to_string(),
                input_schema: json!({"type": "object"}),
            }],
        })
    }

    async fn invoke(
        &self,
        request: ScriptToolInvokeRequest,
    ) -> PortResult<ScriptToolInvokeResponse> {
        self.operations
            .lock()
            .unwrap()
            .push(format!("invoke:{}", request.operation_id));
        Ok(ScriptToolInvokeResponse {
            output: "sunny".to_string(),
        })
    }

    async fn cancel(&self, target_id: &str, operation_id: &str) -> PortResult<()> {
        self.operations
            .lock()
            .unwrap()
            .push(format!("cancel:{target_id}:{operation_id}"));
        Ok(())
    }

    async fn dispose(&self, target_id: &str) -> PortResult<()> {
        let mut loaded = self.loaded.lock().unwrap();
        if loaded.as_deref() == Some(target_id) {
            *loaded = None;
        }
        drop(loaded);
        self.operations
            .lock()
            .unwrap()
            .push(format!("dispose:{target_id}"));
        Ok(())
    }
}

#[tokio::test]
async fn port_keeps_worker_lifecycle_provider_neutral() {
    let runtime = FakeRuntime::default();
    let availability = runtime.availability().await;
    assert!(matches!(
        availability,
        ScriptToolRuntimeAvailability::Available { .. }
    ));

    let loaded = runtime
        .load(ScriptToolLoadRequest {
            target_id: "target-1".to_string(),
            revision: "v1".to_string(),
            module_source: "export default {}".to_string(),
            module_url: "file:///workspace/tool.js".to_string(),
            working_directory: "/workspace".to_string(),
            expected_tools: vec![ScriptToolExpectedExport {
                export_name: "default".to_string(),
                tool_name: "weather".to_string(),
            }],
        })
        .await
        .unwrap();
    assert_eq!(loaded.tools[0].name, "weather");

    runtime
        .invoke(ScriptToolInvokeRequest {
            target_id: "target-1".to_string(),
            revision: "v1".to_string(),
            export_name: "default".to_string(),
            operation_id: "operation-1".to_string(),
            arguments: json!({"location": "Shanghai"}),
            workspace_root: Some("/workspace".to_string()),
            worktree_root: Some("/workspace".to_string()),
            session_id: Some("session-1".to_string()),
        })
        .await
        .unwrap();
    runtime.cancel("target-1", "operation-1").await.unwrap();
    runtime.dispose("target-1").await.unwrap();

    assert_eq!(
        runtime.operations.into_inner().unwrap(),
        vec![
            "load:target-1",
            "invoke:operation-1",
            "cancel:target-1:operation-1",
            "dispose:target-1",
        ]
    );
}

#[test]
fn load_contract_carries_no_ecosystem_specific_fields() {
    let value = serde_json::to_value(ScriptToolLoadRequest {
        target_id: "target-1".to_string(),
        revision: "v1".to_string(),
        module_source: "export default {}".to_string(),
        module_url: "file:///workspace/tool.js".to_string(),
        working_directory: "/workspace".to_string(),
        expected_tools: vec![ScriptToolExpectedExport {
            export_name: "default".to_string(),
            tool_name: "weather".to_string(),
        }],
    })
    .unwrap();

    assert_eq!(value["targetId"], "target-1");
    assert!(value.get("ecosystem").is_none());
    assert!(value.get("opencode").is_none());
}
