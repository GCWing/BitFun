use async_trait::async_trait;
use bitfun_runtime_ports::{
    PluginDispatchEnvelope, PluginResponseEnvelope, PluginRuntimeReadRequest,
    PluginRuntimeReadResponse, PortResult,
};

#[async_trait]
pub trait PluginHostAdapter: Send + Sync {
    fn adapter_id(&self) -> &str;

    async fn read_plugins(
        &self,
        request: PluginRuntimeReadRequest,
    ) -> PortResult<PluginRuntimeReadResponse>;

    async fn dispatch(
        &self,
        envelope: PluginDispatchEnvelope,
    ) -> PortResult<PluginResponseEnvelope>;
}
