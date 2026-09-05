use async_trait::async_trait;
use openbitfun_runtime_ports::{
    PluginDispatchEnvelope, PluginResponseEnvelope, PluginRuntimeAvailability,
    PluginRuntimeReadRequest, PluginRuntimeReadResponse, PortResult,
};

#[async_trait]
pub trait PluginRuntimeAdapter: Send + Sync {
    fn adapter_id(&self) -> &str;

    fn availability(&self) -> PluginRuntimeAvailability;

    async fn read_plugins(
        &self,
        request: PluginRuntimeReadRequest,
    ) -> PortResult<PluginRuntimeReadResponse>;

    async fn dispatch(
        &self,
        envelope: PluginDispatchEnvelope,
    ) -> PortResult<PluginResponseEnvelope>;
}
