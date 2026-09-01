/**
 * Source aliases can point outside the application's watched root. Asset
 * loaders do not necessarily register those files, so an edited SVG can leave
 * its old inline module cached even though the raw file is already up to date.
 *
 * @param {string} sourceDirectory
 * @returns {import("./watch-source.mjs").SourceWatchPlugin}
 */
export function watchSourcePlugin(sourceDirectory) {
  return {
    name: "bitfun:watch-ui-source",
    apply: "serve",
    configureServer(server) {
      server.watcher.add(sourceDirectory);
    },
  };
}
