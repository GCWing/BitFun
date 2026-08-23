//! Shared, config-backed BitFun product control.
//!
//! Product surfaces may provide extra adapters for presentation and native
//! providers, but ordinary BitFun settings are owned by the shared
//! [`ConfigService`]. Keeping their read/write behavior here lets Desktop,
//! CLI, and other headless product hosts execute the same catalog handlers.

use crate::service::config::types::{GlobalConfig, MemoriesConfig};
use crate::service::config::ConfigService;
use bitfun_product_domains::product_control::{
    validate_option_value, ProductCapabilityOption, ProductCapabilityOptionHandler,
};
use serde_json::{Map, Value};

#[derive(Debug, Clone, PartialEq)]
pub struct AppliedProductConfigOption {
    pub changed_path: String,
    pub effective_value: Value,
}

fn read_nested_value<'a>(value: &'a Value, field: &str) -> Option<&'a Value> {
    let mut current = value;
    for segment in field.split('.') {
        current = current.as_object()?.get(segment)?;
    }
    Some(current)
}

fn set_nested_value(value: &mut Value, field: &str, next_value: Value) -> Result<(), String> {
    let segments: Vec<&str> = field
        .split('.')
        .filter(|segment| !segment.is_empty())
        .collect();
    let Some((last, parents)) = segments.split_last() else {
        return Err("A product-control config field cannot be empty".to_string());
    };
    let mut current = value;
    for segment in parents {
        if !current.is_object() {
            *current = Value::Object(Map::new());
        }
        current = current
            .as_object_mut()
            .expect("object was initialized")
            .entry((*segment).to_string())
            .or_insert_with(|| Value::Object(Map::new()));
    }
    if !current.is_object() {
        *current = Value::Object(Map::new());
    }
    current
        .as_object_mut()
        .expect("object was initialized")
        .insert((*last).to_string(), next_value);
    Ok(())
}

fn validate_config_semantics(path: &str, value: &Value) -> Result<(), String> {
    if path == "memories" {
        let memories: MemoriesConfig = serde_json::from_value(value.clone())
            .map_err(|error| format!("Invalid memory settings: {error}"))?;
        if memories.max_rollout_age_days > memories.max_unused_days {
            return Err("Memory rollout age must not exceed unused-memory retention".to_string());
        }
    }
    Ok(())
}

/// Read an option implemented by the shared BitFun configuration service.
///
/// `Ok(None)` means the semantic catalog deliberately routed the option to a
/// product-host provider instead of shared config.
pub async fn read_config_backed_option(
    config_service: &ConfigService,
    option: &ProductCapabilityOption,
) -> Result<Option<Value>, String> {
    let value = match &option.handler {
        ProductCapabilityOptionHandler::Config { path } => config_service
            .get_config(Some(path))
            .await
            .map_err(|error| error.to_string())?,
        ProductCapabilityOptionHandler::MergeConfig { path, fields } => {
            let current: Value = config_service
                .get_config(Some(path))
                .await
                .map_err(|error| error.to_string())?;
            let values: Vec<Value> = fields
                .iter()
                .map(|field| {
                    read_nested_value(&current, field)
                        .cloned()
                        .unwrap_or(Value::Null)
                })
                .collect();
            if values.len() == 1 || values.windows(2).all(|pair| pair[0] == pair[1]) {
                values.into_iter().next().unwrap_or(Value::Null)
            } else {
                Value::Object(fields.iter().cloned().zip(values).collect::<Map<_, _>>())
            }
        }
        ProductCapabilityOptionHandler::AppearanceSelection => config_service
            .get_config(Some("appearance.selection"))
            .await
            .map_err(|error| error.to_string())?,
        ProductCapabilityOptionHandler::Language => config_service
            .get_config(Some("app.language"))
            .await
            .map_err(|error| error.to_string())?,
        ProductCapabilityOptionHandler::FlowChatPermissionModeControl => {
            let config: GlobalConfig = config_service
                .get_config(None)
                .await
                .map_err(|error| error.to_string())?;
            Value::Bool(config.app.flow_chat.show_permission_mode_control)
        }
        ProductCapabilityOptionHandler::Provider { .. } => return Ok(None),
    };
    Ok(Some(value))
}

/// Apply an option implemented by the shared BitFun configuration service and
/// read the persisted effective value back through the same handler.
///
/// `Ok(None)` means a product-host provider owns the option.
pub async fn configure_config_backed_option(
    config_service: &ConfigService,
    option: &ProductCapabilityOption,
    value: &Value,
) -> Result<Option<AppliedProductConfigOption>, String> {
    validate_option_value(&option.value_schema, value)?;
    let changed_path = match &option.handler {
        ProductCapabilityOptionHandler::Config { path } => {
            config_service
                .set_config(path, value.clone())
                .await
                .map_err(|error| error.to_string())?;
            path.clone()
        }
        ProductCapabilityOptionHandler::MergeConfig { path, fields } => {
            let mut current: Value = config_service
                .get_config(Some(path))
                .await
                .map_err(|error| error.to_string())?;
            for field in fields {
                set_nested_value(&mut current, field, value.clone())?;
            }
            validate_config_semantics(path, &current)?;
            config_service
                .set_config(path, current)
                .await
                .map_err(|error| error.to_string())?;
            path.clone()
        }
        ProductCapabilityOptionHandler::AppearanceSelection => {
            config_service
                .set_config("appearance.selection", value.clone())
                .await
                .map_err(|error| error.to_string())?;
            "appearance.selection".to_string()
        }
        ProductCapabilityOptionHandler::Language => {
            config_service
                .set_config("app.language", value.clone())
                .await
                .map_err(|error| error.to_string())?;
            "app.language".to_string()
        }
        ProductCapabilityOptionHandler::FlowChatPermissionModeControl => {
            config_service
                .set_config("app.flow_chat.show_permission_mode_control", value.clone())
                .await
                .map_err(|error| error.to_string())?;
            "app.flow_chat.show_permission_mode_control".to_string()
        }
        ProductCapabilityOptionHandler::Provider { .. } => return Ok(None),
    };
    let effective_value = read_config_backed_option(config_service, option)
        .await?
        .ok_or_else(|| "Shared config option unexpectedly became provider-backed".to_string())?;
    Ok(Some(AppliedProductConfigOption {
        changed_path,
        effective_value,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::PathManager;
    use crate::service::config::types::GlobalConfig;
    use crate::service::config::{ConfigManagerSettings, ConfigService};
    use bitfun_product_domains::product_control::{
        capability as product_capability, catalog, ProductControlValueSchema,
        ProductControlValueType,
    };
    use std::sync::Arc;

    async fn test_service(name: &str) -> (ConfigService, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("tempdir");
        let path_manager = Arc::new(PathManager::with_user_root_for_tests(dir.path().join(name)));
        let service = ConfigService::with_settings(ConfigManagerSettings {
            path_manager: Some(path_manager),
            auto_save: true,
            backup_count: 0,
        })
        .await
        .expect("config service");
        (service, dir)
    }

    fn writable_samples(schema: &ProductControlValueSchema, current: Option<&Value>) -> Vec<Value> {
        let mut samples = Vec::new();
        if let Some(values) = &schema.r#enum {
            samples.extend(values.iter().cloned());
        }
        match schema.value_type {
            ProductControlValueType::Boolean => {
                samples.push(Value::Bool(true));
                samples.push(Value::Bool(false));
            }
            ProductControlValueType::String => samples.push(Value::String(
                "x".repeat(schema.min_length.unwrap_or(1).max(1)),
            )),
            ProductControlValueType::Integer => {
                samples.push(Value::from(schema.minimum.unwrap_or(1.0).ceil() as i64));
                if let Some(maximum) = schema.maximum {
                    samples.push(Value::from(maximum.floor() as i64));
                }
            }
            ProductControlValueType::Number => {
                samples.push(Value::from(schema.minimum.unwrap_or(1.0)));
                if let Some(maximum) = schema.maximum {
                    samples.push(Value::from(maximum));
                }
            }
            ProductControlValueType::Object => samples.push(serde_json::json!({})),
            ProductControlValueType::Array => samples.push(serde_json::json!([])),
        }
        if schema.nullable {
            samples.push(Value::Null);
        }
        if let Some(current) = current {
            samples.push(current.clone());
        }
        samples.retain(|sample| validate_option_value(schema, sample).is_ok());
        samples.dedup();
        samples
    }

    fn assert_config_binding(root: &Value, path: &str, schema: &ProductControlValueSchema) {
        let current = read_nested_value(root, path);
        if let Some(current) = current {
            assert!(
                validate_option_value(schema, current).is_ok(),
                "default value at {path} does not satisfy its product-control schema: {current}"
            );
        }

        let samples = writable_samples(schema, current);
        for sample in &samples {
            let mut candidate = root.clone();
            set_nested_value(&mut candidate, path, sample.clone()).unwrap();
            let Ok(typed) = serde_json::from_value::<GlobalConfig>(candidate) else {
                continue;
            };
            let serialized = serde_json::to_value(typed).unwrap();
            if read_nested_value(&serialized, path) == Some(sample) {
                return;
            }
        }
        panic!(
            "product-control config path is not consumed by typed GlobalConfig: {path}; valid samples: {samples:?}"
        );
    }

    #[tokio::test]
    async fn config_handler_round_trips_through_the_shared_service() {
        let (service, _dir) = test_service("product-control-round-trip").await;
        let capability = product_capability("setting.tools.execution").unwrap();
        let option = capability
            .options
            .iter()
            .find(|option| option.id == "deferred-tool-loading")
            .unwrap();

        let applied = configure_config_backed_option(&service, option, &Value::Bool(false))
            .await
            .unwrap()
            .expect("config-backed option");
        assert_eq!(applied.changed_path, "ai.enable_deferred_tool_loading");
        assert_eq!(applied.effective_value, Value::Bool(false));
        assert_eq!(
            read_config_backed_option(&service, option).await.unwrap(),
            Some(Value::Bool(false))
        );
    }

    #[tokio::test]
    async fn every_catalog_config_option_is_readable_from_default_config() {
        let (service, _dir) = test_service("product-control-default-readback").await;
        for option in catalog()
            .unwrap()
            .capabilities
            .iter()
            .flat_map(|capability| &capability.options)
        {
            if matches!(
                option.handler,
                ProductCapabilityOptionHandler::Provider { .. }
            ) {
                continue;
            }
            let value = read_config_backed_option(&service, option)
                .await
                .unwrap_or_else(|error| panic!("{} is unreadable: {error}", option.id));
            assert!(
                value.is_some(),
                "{} unexpectedly requires a host",
                option.id
            );
        }
    }

    #[tokio::test]
    async fn every_catalog_config_option_can_be_applied_and_read_back() {
        let (service, _dir) = test_service("product-control-all-options-round-trip").await;
        for capability in &catalog().unwrap().capabilities {
            for option in &capability.options {
                if matches!(
                    option.handler,
                    ProductCapabilityOptionHandler::Provider { .. }
                ) {
                    continue;
                }
                let current = read_config_backed_option(&service, option)
                    .await
                    .unwrap_or_else(|error| {
                        panic!(
                            "{}.{} initial read failed: {error}",
                            capability.id, option.id
                        )
                    })
                    .unwrap_or_else(|| {
                        panic!(
                            "{}.{} unexpectedly requires a host",
                            capability.id, option.id
                        )
                    });
                let candidates = writable_samples(&option.value_schema, Some(&current));
                let mut failures = Vec::new();
                let mut applied = false;
                for candidate in candidates {
                    match configure_config_backed_option(&service, option, &candidate).await {
                        Ok(Some(result)) if result.effective_value == candidate => {
                            applied = true;
                            break;
                        }
                        Ok(Some(result)) => failures.push(format!(
                            "{candidate} read back as {}",
                            result.effective_value
                        )),
                        Ok(None) => {
                            failures.push(format!("{candidate} unexpectedly required host"))
                        }
                        Err(error) => failures.push(format!("{candidate}: {error}")),
                    }
                }
                assert!(
                    applied,
                    "{}.{} has no shared config value that round-trips: {}",
                    capability.id,
                    option.id,
                    failures.join("; ")
                );
            }
        }
    }

    #[test]
    fn every_catalog_config_option_binds_to_typed_global_config() {
        let root = serde_json::to_value(GlobalConfig::default()).unwrap();
        for option in catalog()
            .unwrap()
            .capabilities
            .iter()
            .flat_map(|capability| &capability.options)
        {
            match &option.handler {
                ProductCapabilityOptionHandler::Config { path } => {
                    assert_config_binding(&root, path, &option.value_schema);
                }
                ProductCapabilityOptionHandler::MergeConfig { path, fields } => {
                    for field in fields {
                        assert_config_binding(
                            &root,
                            &format!("{path}.{field}"),
                            &option.value_schema,
                        );
                    }
                    let current = read_nested_value(&root, path)
                        .unwrap_or_else(|| panic!("product-control merge path is absent: {path}"));
                    validate_config_semantics(path, current).unwrap();
                }
                ProductCapabilityOptionHandler::AppearanceSelection => {
                    assert_config_binding(&root, "appearance.selection", &option.value_schema);
                }
                ProductCapabilityOptionHandler::Language => {
                    assert_config_binding(&root, "app.language", &option.value_schema);
                }
                ProductCapabilityOptionHandler::FlowChatPermissionModeControl => {
                    let current = Value::Bool(
                        GlobalConfig::default()
                            .app
                            .flow_chat
                            .show_permission_mode_control,
                    );
                    assert!(validate_option_value(&option.value_schema, &current).is_ok());
                }
                ProductCapabilityOptionHandler::Provider { .. } => {}
            }
        }
    }
}
