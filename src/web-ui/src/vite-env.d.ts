/// <reference types="vite/client" />

declare module 'virtual:openbitfun-canvas-runtime-bundle' {
  const bundle: {
    js: string;
    css: string;
  };
  export const openbitfunCanvasRuntimeBundle: typeof bundle;
  export default bundle;
}
