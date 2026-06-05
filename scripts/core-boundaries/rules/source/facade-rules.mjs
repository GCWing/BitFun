// Boundary rules for source ownership, facades, and required owner content.

export const facadeOnlyFiles = [
  {
    path: 'src/crates/assembly/core/src/infrastructure/filesystem/mod.rs',
    importPrefix: 'bitfun_services_core::filesystem',
    reason: 'core filesystem infrastructure facade must only re-export the services-core owner crate',
  },
  {
    path: 'src/crates/assembly/core/src/service/filesystem/listing.rs',
    importPrefix: 'bitfun_services_core::filesystem',
    reason: 'core filesystem listing facade must only re-export the services-core owner crate',
  },
  {
    path: 'src/crates/assembly/core/src/service/filesystem/types.rs',
    importPrefix: 'bitfun_services_core::filesystem',
    reason: 'core filesystem DTO facade must only re-export the services-core owner crate',
  },
  {
    path: 'src/crates/assembly/core/src/service/git/git_service.rs',
    importPrefix: 'bitfun_services_integrations::git',
    reason: 'core git service facade must only re-export the integrations owner crate',
  },
  {
    path: 'src/crates/assembly/core/src/service/git/git_types.rs',
    importPrefix: 'bitfun_services_integrations::git',
    reason: 'core git types facade must only re-export the integrations owner crate',
  },
  {
    path: 'src/crates/assembly/core/src/service/git/git_utils.rs',
    importPrefix: 'bitfun_services_integrations::git',
    reason: 'core git utils facade must only re-export the integrations owner crate',
  },
  {
    path: 'src/crates/assembly/core/src/service/git/graph.rs',
    importPrefix: 'bitfun_services_integrations::git',
    reason: 'core git graph facade must only re-export the integrations owner crate',
  },
  {
    path: 'src/crates/assembly/core/src/service/remote_ssh/types.rs',
    importPrefix: 'bitfun_services_integrations::remote_ssh',
    reason: 'core remote SSH types facade must only re-export the integrations owner crate',
  },
  {
    path: 'src/crates/assembly/core/src/service/mcp/tool_info.rs',
    importPrefix: 'bitfun_services_integrations::mcp',
    reason: 'core MCP tool info facade must only re-export the integrations owner crate',
  },
  {
    path: 'src/crates/assembly/core/src/service/mcp/tool_name.rs',
    importPrefix: 'bitfun_services_integrations::mcp',
    reason: 'core MCP tool name facade must only re-export the integrations owner crate',
  },
  {
    path: 'src/crates/assembly/core/src/service/mcp/protocol/types.rs',
    importPrefix: 'bitfun_services_integrations::mcp',
    reason: 'core MCP protocol types facade must only re-export the integrations owner crate',
  },
  {
    path: 'src/crates/assembly/core/src/service/mcp/protocol/transport.rs',
    importPrefix: 'bitfun_services_integrations::mcp::protocol',
    reason: 'core MCP stdio transport facade must only re-export the integrations owner crate',
  },
  {
    path: 'src/crates/assembly/core/src/service/mcp/protocol/transport_remote.rs',
    importPrefix: 'bitfun_services_integrations::mcp::protocol',
    reason: 'core MCP remote transport facade must only re-export the integrations owner crate',
  },
  {
    path: 'src/crates/assembly/core/src/service/mcp/server/connection.rs',
    importPrefix: 'bitfun_services_integrations::mcp::server',
    reason: 'core MCP connection facade must only re-export the integrations owner crate',
  },
  {
    path: 'src/crates/assembly/core/src/service/mcp/config/location.rs',
    importPrefix: 'bitfun_services_integrations::mcp',
    reason: 'core MCP config location facade must only re-export the integrations owner crate',
  },
  {
    path: 'src/crates/assembly/core/src/service/mcp/adapter/resource.rs',
    importPrefix: 'bitfun_services_integrations::mcp',
    reason: 'core MCP resource adapter facade must only re-export the integrations owner crate',
  },
  {
    path: 'src/crates/assembly/core/src/service/mcp/adapter/prompt.rs',
    importPrefix: 'bitfun_services_integrations::mcp',
    reason: 'core MCP prompt adapter facade must only re-export the integrations owner crate',
  },
  {
    path: 'src/crates/assembly/core/src/service/announcement/types.rs',
    importPrefix: 'bitfun_services_integrations::announcement',
    reason: 'core announcement types facade must only re-export the integrations owner crate',
  },
];
