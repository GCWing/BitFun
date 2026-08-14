import { currentMonitor, getCurrentWindow } from '@tauri-apps/api/window';
import { LogicalSize, PhysicalPosition, PhysicalSize } from '@tauri-apps/api/dpi';
import { isOpenHarmonyRuntime } from '@/infrastructure/runtime';
import { workspaceAPI } from '@/infrastructure/api';
import type { ToolbarMonitorGeometry } from './toolbarWindowGeometry';

export interface Point2D {
  x: number;
  y: number;
}

export interface Size2D {
  width: number;
  height: number;
}

export interface WindowOps {
  outerPosition(): Promise<Point2D>;
  innerSize(): Promise<Size2D>;
  outerSize(): Promise<Size2D>;
  isMaximized(): Promise<boolean>;
  isDecorated(): Promise<boolean | undefined>;
  unmaximize(): Promise<void>;
  setMinSize(size: Size2D | null): Promise<void>;
  setMinSizeLogical(size: Size2D): Promise<void>;
  setAlwaysOnTop(top: boolean): Promise<void>;
  setSize(size: Size2D): Promise<void>;
  setSizeLogical(size: Size2D): Promise<void>;
  setPosition(pos: Point2D): Promise<void>;
  setResizable(resizable: boolean): Promise<void>;
  setSkipTaskbar(skip: boolean): Promise<void>;
  setDecorations(visible: boolean): Promise<void>;
  setTitleBarOverlay(): Promise<void>;
  maximize(): Promise<void>;
  center(): Promise<void>;
  setFocus(): Promise<void>;
  currentMonitor(): Promise<ToolbarMonitorGeometry | null>;
}

const tauriOps = (): WindowOps => {
  const win = getCurrentWindow();
  return {
    async outerPosition(): Promise<Point2D> {
      const p = await win.outerPosition();
      return { x: p.x, y: p.y };
    },
    async innerSize(): Promise<Size2D> {
      const s = await win.innerSize();
      return { width: s.width, height: s.height };
    },
    async outerSize(): Promise<Size2D> {
      const s = await win.outerSize();
      return { width: s.width, height: s.height };
    },
    async isMaximized(): Promise<boolean> {
      return await win.isMaximized();
    },
    async isDecorated(): Promise<boolean | undefined> {
      try {
        const candidate = win as unknown as { isDecorated?: () => Promise<boolean> };
        if (typeof candidate.isDecorated === 'function') {
          return await candidate.isDecorated();
        }
      } catch {
        // ignore
      }
      return undefined;
    },
    async unmaximize(): Promise<void> {
      await win.unmaximize();
    },
    async setMinSize(size: Size2D | null): Promise<void> {
      if (size === null) {
        await win.setMinSize(null);
      } else {
        await win.setMinSize(new PhysicalSize(size.width, size.height));
      }
    },
    async setMinSizeLogical(size: Size2D): Promise<void> {
      await win.setMinSize(new LogicalSize(size.width, size.height));
    },
    async setAlwaysOnTop(top: boolean): Promise<void> {
      await win.setAlwaysOnTop(top);
    },
    async setSize(size: Size2D): Promise<void> {
      await win.setSize(new PhysicalSize(size.width, size.height));
    },
    async setSizeLogical(size: Size2D): Promise<void> {
      await win.setSize(new LogicalSize(size.width, size.height));
    },
    async setPosition(pos: Point2D): Promise<void> {
      await win.setPosition(new PhysicalPosition(pos.x, pos.y));
    },
    async setResizable(resizable: boolean): Promise<void> {
      await win.setResizable(resizable);
    },
    async setSkipTaskbar(skip: boolean): Promise<void> {
      await win.setSkipTaskbar(skip);
    },
    async setDecorations(visible: boolean): Promise<void> {
      await win.setDecorations(visible);
    },
    async setTitleBarOverlay(): Promise<void> {
      await win.setTitleBarStyle('overlay');
    },
    async maximize(): Promise<void> {
      await win.maximize();
    },
    async center(): Promise<void> {
      await win.center();
    },
    async setFocus(): Promise<void> {
      await win.setFocus();
    },
    async currentMonitor(): Promise<ToolbarMonitorGeometry | null> {
      const m = await currentMonitor();
      if (!m) return null;
      return {
        position: { x: m.position.x, y: m.position.y },
        size: { width: m.size.width, height: m.size.height },
        scaleFactor: m.scaleFactor,
      };
    },
  };
};

const logicalToPhysical = async (size: Size2D): Promise<Size2D> => {
  const monitor = await workspaceAPI.currentMonitorOhos();
  const scale = monitor.scaleFactor > 0 ? monitor.scaleFactor : 1;
  return {
    width: Math.max(1, Math.round(size.width * scale)),
    height: Math.max(1, Math.round(size.height * scale)),
  };
};

const ohosOps = (): WindowOps => ({
  async outerPosition(): Promise<Point2D> {
    return await workspaceAPI.outerPositionOhos();
  },
  async innerSize(): Promise<Size2D> {
    return await workspaceAPI.innerSizeOhos();
  },
  async outerSize(): Promise<Size2D> {
    return await workspaceAPI.outerSizeOhos();
  },
  async isMaximized(): Promise<boolean> {
    return await workspaceAPI.window_is_maximized();
  },
  async isDecorated(): Promise<boolean | undefined> {
    return false;
  },
  async unmaximize(): Promise<void> {
    await workspaceAPI.unmaximizeOhos();
  },
  async setMinSize(size: Size2D | null): Promise<void> {
    if (size !== null) {
      await workspaceAPI.setMinSizeOhos(size.width, size.height);
    }
  },
  async setMinSizeLogical(size: Size2D): Promise<void> {
    const physical = await logicalToPhysical(size);
    await workspaceAPI.setMinSizeOhos(physical.width, physical.height);
  },
  async setAlwaysOnTop(top: boolean): Promise<void> {
    await workspaceAPI.setAlwaysOnTopOhos(top);
  },
  async setSize(size: Size2D): Promise<void> {
    await workspaceAPI.setWindowSizeOhos(size.width, size.height);
  },
  async setSizeLogical(size: Size2D): Promise<void> {
    const physical = await logicalToPhysical(size);
    await workspaceAPI.setWindowSizeOhos(physical.width, physical.height);
  },
  async setPosition(pos: Point2D): Promise<void> {
    await workspaceAPI.setWindowPositionOhos(pos.x, pos.y);
  },
  async setResizable(resizable: boolean): Promise<void> {
    await workspaceAPI.setResizableOhos(resizable);
  },
  async setSkipTaskbar(skip: boolean): Promise<void> {
    await workspaceAPI.setSkipTaskbarOhos(skip);
  },
  async setDecorations(visible: boolean): Promise<void> {
    await workspaceAPI.setDecorationsOhos(visible);
  },
  async setTitleBarOverlay(): Promise<void> {
    // No-op on OpenHarmony; the native ArkUI host owns the window chrome.
  },
  async maximize(): Promise<void> {
    await workspaceAPI.maximizeOhos();
  },
  async center(): Promise<void> {
    await workspaceAPI.centerOhos();
  },
  async setFocus(): Promise<void> {
    await workspaceAPI.setFocusOhos();
  },
  async currentMonitor(): Promise<ToolbarMonitorGeometry | null> {
    const m = await workspaceAPI.currentMonitorOhos();
    const avoid = m.avoidArea;
    const hasAvoid = avoid && (avoid.top > 0 || avoid.bottom > 0 || avoid.left > 0 || avoid.right > 0);
    const workArea = hasAvoid
      ? {
          position: { x: avoid.left, y: avoid.top },
          size: {
            width: Math.max(0, m.width - avoid.left - avoid.right),
            height: Math.max(0, m.height - avoid.top - avoid.bottom),
          },
        }
      : undefined;
    return {
      position: { x: 0, y: 0 },
      size: { width: m.width, height: m.height },
      scaleFactor: m.scaleFactor,
      workArea,
    };
  },
});

export const createWindowOps = (): WindowOps =>
  isOpenHarmonyRuntime() ? ohosOps() : tauriOps();

