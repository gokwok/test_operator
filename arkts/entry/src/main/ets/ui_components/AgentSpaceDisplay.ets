import { display } from '@kit.ArkUI';

export type AgentSpaceDisplayMode = 'display' | 'control';

export type AgentSpacePointerAction = 'down' | 'move' | 'up' | 'cancel';

export interface AgentSpacePointerEvent {
  action: AgentSpacePointerAction;
  xNorm: number;
  yNorm: number;
  pointerId?: number;
}

export interface AgentSpaceDisplaySurfaceInfo {
  surfaceId: string;
  surfaceWidth: number;
  surfaceHeight: number;
}

export class AgentSpaceDisplayCallbacks {
  onSurfaceReady?: (info: AgentSpaceDisplaySurfaceInfo) => void;
  onPointerEvent?: (event: AgentSpacePointerEvent) => void;
}

interface SurfaceRect {
  surfaceWidth: number;
  surfaceHeight: number;
  offsetX: number;
}

class SurfaceSize {
  width: number;
  height: number;

  constructor(width: number, height: number) {
    this.width = width;
    this.height = height;
  }
}

@Component
export struct AgentSpaceDisplay {
  @Prop mode: AgentSpaceDisplayMode = 'display';
  @Prop surfaceWidth: number = -1;
  @Prop surfaceHeight: number = -1;
  @Prop callbacks: AgentSpaceDisplayCallbacks | null = null;

  private xComponentController: XComponentController = new XComponentController();
  private viewWidth: number = 0;
  private viewHeight: number = 0;

  build() {
    XComponent({
      type: XComponentType.SURFACE,
      controller: this.xComponentController
    })
      .onLoad(() => {
        const size = this.resolveSurfaceSize();
        const rect: SurfaceRect = {
          surfaceWidth: size.width,
          surfaceHeight: size.height,
          offsetX: 0,
        };
        this.xComponentController.setXComponentSurfaceRect(rect);

        const info: AgentSpaceDisplaySurfaceInfo = {
          surfaceId: this.xComponentController.getXComponentSurfaceId(),
          surfaceWidth: size.width,
          surfaceHeight: size.height,
        };
        const cb = this.callbacks;
        if (cb && cb.onSurfaceReady) {
          cb.onSurfaceReady(info);
        }
      })
      .onAreaChange((_oldValue, newValue): void => {
        this.viewWidth = Number(newValue.width);
        this.viewHeight = Number(newValue.height);
      })
      .onTouch((event) => {
        const cb = this.callbacks;
        if (this.mode !== 'control' || !cb || !cb.onPointerEvent) {
          return;
        }
        const touch = this.getPrimaryTouch(event);
        if (!touch) {
          return;
        }
        const action = this.mapAction(event.type);
        if (!action) {
          return;
        }
        const xNorm = this.normalize(touch.windowX, this.viewWidth);
        const yNorm = this.normalize(touch.windowY, this.viewHeight);
        const payload: AgentSpacePointerEvent = {
          action,
          xNorm,
          yNorm,
          pointerId: touch.id,
        };
        cb.onPointerEvent(payload);
      })
      .width('100%')
      .height('100%')
  }

  private resolveSurfaceSize(): SurfaceSize {
    if (this.surfaceWidth > 0 && this.surfaceHeight > 0) {
      return new SurfaceSize(this.surfaceWidth, this.surfaceHeight);
    }
    const d = display.getDefaultDisplaySync();
    return new SurfaceSize(d.width, d.height);
  }

  private normalize(value: number, limit: number): number {
    if (!Number.isFinite(value) || limit <= 0) {
      return 0;
    }
    const clamped = Math.min(Math.max(value / limit, 0), 1);
    return clamped;
  }

  private mapAction(action: TouchType): AgentSpacePointerAction | null {
    if (action === TouchType.Down) {
      return 'down';
    }
    if (action === TouchType.Move) {
      return 'move';
    }
    if (action === TouchType.Up) {
      return 'up';
    }
    if (action === TouchType.Cancel) {
      return 'cancel';
    }
    return null;
  }

  private getPrimaryTouch(event: TouchEvent): TouchObject | null {
    const changed = event.changedTouches;
    if (changed && changed.length > 0) {
      return changed[0];
    }
    const touches = event.touches;
    if (touches && touches.length > 0) {
      return touches[0];
    }
    return null;
  }
}
