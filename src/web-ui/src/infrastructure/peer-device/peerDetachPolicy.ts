export function shouldSurfacePeerDetachFailure(reason?: string): boolean {
  return reason !== 'peer_offline' && reason !== 'rpc_failures';
}
