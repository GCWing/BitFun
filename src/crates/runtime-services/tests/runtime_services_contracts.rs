use std::sync::Arc;

use bitfun_runtime_ports::FileSystemPort;
use bitfun_runtime_ports::RuntimeServiceCapability;
use bitfun_runtime_services::test_support::{FakeRuntimePort, FakeRuntimeServicesProvider};
use bitfun_runtime_services::{
    CapabilityAvailability, RuntimeServicesBuilder, RuntimeServicesError, RuntimeServicesProvider,
    RuntimeServicesRegistry,
};

#[test]
fn builder_requires_mandatory_runtime_services() {
    let error = RuntimeServicesBuilder::new().build().unwrap_err();

    assert_eq!(
        error,
        RuntimeServicesError::MissingRequired {
            capability: RuntimeServiceCapability::FileSystem,
        }
    );
}

#[test]
fn fake_provider_registers_required_and_remote_services_through_registry() {
    let registry = RuntimeServicesRegistry::new()
        .with_provider(FakeRuntimeServicesProvider::with_all_required().with_all_remote());
    let services = registry
        .build(RuntimeServicesBuilder::new())
        .expect("fake provider should satisfy runtime services");

    assert!(services.has_capability(RuntimeServiceCapability::FileSystem));
    assert!(services.has_capability(RuntimeServiceCapability::Workspace));
    assert!(services.has_capability(RuntimeServiceCapability::SessionStore));
    assert!(services.has_capability(RuntimeServiceCapability::Permission));
    assert!(services.has_capability(RuntimeServiceCapability::Events));
    assert!(services.has_capability(RuntimeServiceCapability::Clock));
    assert!(services.has_capability(RuntimeServiceCapability::RemoteConnection));
    assert!(services.has_capability(RuntimeServiceCapability::RemoteWorkspace));
    assert!(services.has_capability(RuntimeServiceCapability::RemoteProjection));
    assert!(services.has_capability(RuntimeServiceCapability::RemoteCapabilities));
}

#[test]
fn missing_optional_capability_returns_typed_unsupported_error() {
    let services = FakeRuntimeServicesProvider::with_all_required()
        .build_services()
        .expect("required fake services should build");

    let error = services
        .require_capability(RuntimeServiceCapability::RemoteConnection)
        .unwrap_err();

    assert_eq!(
        error,
        RuntimeServicesError::Unsupported {
            capability: RuntimeServiceCapability::RemoteConnection,
        }
    );
}

#[test]
fn capability_availability_reports_optional_service_status_without_side_effects() {
    let services = FakeRuntimeServicesProvider::with_all_required()
        .build_services()
        .expect("required fake services should build");

    assert_eq!(
        services.capability_availability(RuntimeServiceCapability::FileSystem),
        CapabilityAvailability {
            capability: RuntimeServiceCapability::FileSystem,
            available: true,
        }
    );
    assert_eq!(
        services.capability_availability(RuntimeServiceCapability::RemoteWorkspace),
        CapabilityAvailability {
            capability: RuntimeServiceCapability::RemoteWorkspace,
            available: false,
        }
    );
}

#[test]
fn builder_rejects_port_registered_under_the_wrong_capability() {
    let mismatched_filesystem: Arc<dyn FileSystemPort> =
        Arc::new(FakeRuntimePort::new(RuntimeServiceCapability::Git));
    let builder = FakeRuntimeServicesProvider::with_all_required()
        .register(RuntimeServicesBuilder::new())
        .with_filesystem(mismatched_filesystem);

    let error = builder.build().unwrap_err();

    assert_eq!(
        error,
        RuntimeServicesError::CapabilityMismatch {
            expected: RuntimeServiceCapability::FileSystem,
            actual: RuntimeServiceCapability::Git,
        }
    );
}
