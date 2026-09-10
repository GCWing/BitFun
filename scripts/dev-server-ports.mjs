export function resolveDevServerPorts(env = process.env) {
  const port = Number(env.OPENBITFUN_DEV_PORT || 1422);
  const hmrPort = Number(env.OPENBITFUN_DEV_HMR_PORT || port - 1);
  if (![port, hmrPort].every(value => Number.isInteger(value) && value >= 1024 && value <= 65535)
    || port === hmrPort) {
    throw new Error('Development HTTP and HMR ports must be distinct integers between 1024 and 65535');
  }
  return { port, hmrPort };
}
