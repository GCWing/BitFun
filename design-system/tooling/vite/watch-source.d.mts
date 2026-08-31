// Keep the structural contract independent of each app's Vite installation.
export interface SourceWatchPlugin {
  name: string;
  apply: "serve";
  configureServer(server: { watcher: { add(directory: string): unknown } }): void;
}

export declare function watchSourcePlugin(sourceDirectory: string): SourceWatchPlugin;
