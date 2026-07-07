import './styles/canvas-runtime.scss';
import * as sdkAdapters from './sdk';
import { installBitfunCanvasRuntimeApp } from './CanvasRuntimeApp';

declare global {
  interface Window {
    BitfunCanvasSDKAdapters?: typeof sdkAdapters;
  }
}

window.BitfunCanvasSDKAdapters = sdkAdapters;
installBitfunCanvasRuntimeApp();
