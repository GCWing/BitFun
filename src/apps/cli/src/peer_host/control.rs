//! Peer Mode control-plane subscribers (attach / detach / ping).

use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};

use serde_json::{json, Value};
use tokio::sync::{RwLock, RwLockReadGuard};

use bitfun_core::service::remote_connect::DeviceIdentity;

#[derive(Default)]
struct ControllerRegistry {
    ids: HashSet<String>,
    generation: u64,
}

#[derive(Clone, Copy)]
pub(crate) struct ControllerLease {
    generation: u64,
}

static CONTROL_SUBSCRIBERS: OnceLock<Mutex<ControllerRegistry>> = OnceLock::new();
static CONTROLLER_DELIVERY: OnceLock<RwLock<()>> = OnceLock::new();
const MAX_ATTACHED_CONTROLLERS: usize = 64;

fn control_subscribers() -> &'static Mutex<ControllerRegistry> {
    CONTROL_SUBSCRIBERS.get_or_init(|| Mutex::new(ControllerRegistry::default()))
}

fn controller_delivery() -> &'static RwLock<()> {
    CONTROLLER_DELIVERY.get_or_init(|| RwLock::new(()))
}

pub(crate) async fn attach_controller(device_id: String) -> Result<(), String> {
    if device_id.trim().is_empty() {
        return Err("controller_device_id is required".to_string());
    }
    let _delivery = controller_delivery().write().await;
    let mut registry = control_subscribers()
        .lock()
        .map_err(|_| "Peer controller registry is unavailable".to_string())?;
    if !registry.ids.contains(&device_id) && registry.ids.len() >= MAX_ATTACHED_CONTROLLERS {
        return Err("Peer controller capacity is exhausted".to_string());
    }
    registry.ids.insert(device_id);
    Ok(())
}

pub(crate) async fn detach_controller(device_id: &str) -> bool {
    let _delivery = controller_delivery().write().await;
    control_subscribers()
        .lock()
        .map(|mut registry| detach_from_registry(&mut registry, device_id))
        .unwrap_or(false)
}

pub(crate) async fn retain_online_controllers<'a>(
    online: impl IntoIterator<Item = &'a str>,
) -> bool {
    let online = online.into_iter().collect::<HashSet<_>>();
    let _delivery = controller_delivery().write().await;
    control_subscribers()
        .lock()
        .map(|mut registry| retain_online_in_registry(&mut registry, &online))
        .unwrap_or(false)
}

pub(crate) async fn controller_delivery_lease(
    device_id: &str,
) -> Option<RwLockReadGuard<'static, ()>> {
    let lease = controller_delivery().read().await;
    let attached = control_subscribers()
        .lock()
        .map(|registry| registry.ids.contains(device_id))
        .unwrap_or(false);
    attached.then_some(lease)
}

fn detach_from_registry(registry: &mut ControllerRegistry, device_id: &str) -> bool {
    let was_attached = registry.ids.remove(device_id);
    let lost_all = was_attached && registry.ids.is_empty();
    if lost_all {
        registry.generation = registry.generation.wrapping_add(1);
    }
    lost_all
}

fn retain_online_in_registry(registry: &mut ControllerRegistry, online: &HashSet<&str>) -> bool {
    let had_controllers = !registry.ids.is_empty();
    registry
        .ids
        .retain(|device_id| online.contains(device_id.as_str()));
    let lost_all = had_controllers && registry.ids.is_empty();
    if lost_all {
        registry.generation = registry.generation.wrapping_add(1);
    }
    lost_all
}

pub(crate) fn attached_controller_lease() -> Result<ControllerLease, String> {
    let registry = control_subscribers()
        .lock()
        .map_err(|_| "Peer controller registry is unavailable".to_string())?;
    if registry.ids.is_empty() {
        return Err("A Peer controller must attach before starting a dialog turn".to_string());
    }
    Ok(ControllerLease {
        generation: registry.generation,
    })
}

pub(crate) fn is_controller_lease_current(lease: ControllerLease) -> bool {
    control_subscribers()
        .lock()
        .map(|registry| !registry.ids.is_empty() && registry.generation == lease.generation)
        .unwrap_or(false)
}

pub(crate) fn attached_controllers() -> Vec<String> {
    let mut controllers: Vec<String> = control_subscribers()
        .lock()
        .map(|registry| registry.ids.iter().cloned().collect())
        .unwrap_or_default();
    controllers.sort();
    controllers
}

pub(crate) fn peer_mode_ping_value() -> Value {
    let device_id = DeviceIdentity::from_current_machine()
        .map(|d| d.device_id)
        .unwrap_or_else(|_| "unknown".to_string());
    json!({
        "ok": true,
        "peer": true,
        "device_id": device_id,
    })
}

pub(crate) fn parse_controller_device_id(args: &Value) -> String {
    args.get("controllerDeviceId")
        .or_else(|| args.get("controller_device_id"))
        .or_else(|| {
            args.get("request").and_then(|req| {
                req.get("controllerDeviceId")
                    .or_else(|| req.get("controller_device_id"))
            })
        })
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::sync::Arc;

    use super::{
        attach_controller, controller_delivery_lease, detach_controller, detach_from_registry,
        retain_online_in_registry, ControllerLease, ControllerRegistry,
    };

    fn lease_is_current(registry: &ControllerRegistry, lease: ControllerLease) -> bool {
        !registry.ids.is_empty() && registry.generation == lease.generation
    }

    #[test]
    fn only_the_last_detach_reports_loss_of_all_controllers() {
        let mut registry = ControllerRegistry {
            ids: HashSet::from(["controller-1".to_string(), "controller-2".to_string()]),
            generation: 0,
        };
        let lease = ControllerLease { generation: 0 };

        assert!(!detach_from_registry(&mut registry, "controller-1"));
        assert!(lease_is_current(&registry, lease));
        assert!(detach_from_registry(&mut registry, "controller-2"));
        assert!(!lease_is_current(&registry, lease));
        assert!(!detach_from_registry(&mut registry, "controller-2"));
    }

    #[test]
    fn presence_removal_reports_when_the_last_controller_goes_offline() {
        let mut registry = ControllerRegistry {
            ids: HashSet::from(["controller-1".to_string(), "controller-2".to_string()]),
            generation: 0,
        };
        let first_online = HashSet::from(["controller-1"]);
        assert!(!retain_online_in_registry(&mut registry, &first_online));

        assert!(retain_online_in_registry(&mut registry, &HashSet::new()));
    }

    #[test]
    fn reattach_does_not_revalidate_a_lease_from_before_the_last_detach() {
        let mut registry = ControllerRegistry {
            ids: HashSet::from(["controller-1".to_string()]),
            generation: 7,
        };
        let old_lease = ControllerLease { generation: 7 };

        assert!(detach_from_registry(&mut registry, "controller-1"));
        registry.ids.insert("controller-2".to_string());

        assert!(!lease_is_current(&registry, old_lease));
        assert!(lease_is_current(
            &registry,
            ControllerLease {
                generation: registry.generation,
            }
        ));
    }

    #[tokio::test]
    async fn detach_waits_for_an_in_flight_delivery_lease() {
        let controller_id = "delivery-lease-controller".to_string();
        attach_controller(controller_id.clone())
            .await
            .expect("attach controller");
        let delivery_lease = controller_delivery_lease(&controller_id)
            .await
            .expect("delivery lease");
        let started = Arc::new(tokio::sync::Barrier::new(2));
        let detach_started = Arc::clone(&started);
        let detach_id = controller_id.clone();
        let detach_task = tokio::spawn(async move {
            detach_started.wait().await;
            detach_controller(&detach_id).await
        });

        started.wait().await;
        tokio::task::yield_now().await;
        assert!(!detach_task.is_finished());

        drop(delivery_lease);
        detach_task.await.expect("detach task");
        assert!(controller_delivery_lease(&controller_id).await.is_none());
    }
}
