/// <reference types="vite/client" />

declare module 'virtual:bitfun-canvas-runtime-bundle' {
  const bundle: {
    js: string;
    css: string;
  };
  export const bitfunCanvasRuntimeBundle: typeof bundle;
  export default bundle;
}
