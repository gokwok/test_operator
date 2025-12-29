import { display } from '@kit.ArkUI';
import { Action } from '@ohos.multimodalInput.touchEvent';
import type { TouchEvent } from '@ohos.multimodalInput.touchEvent';

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

interface SurfaceRect {
  surfaceWidth: number;
  surfaceHeight: number;
  offsetX: number;
}

@Component
export struct AgentSpaceDisplay {
  @Prop mode: AgentSpaceDisplayMode = 'display';
  @Prop surfaceWidth: number = -1;
  @Prop surfaceHeight: number = -1;
  @Prop onSurfaceReady?: (info: AgentSpaceDisplaySurfaceInfo) => void;
  @Prop onPointerEvent?: (event: AgentSpacePointerEvent) => void;

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
        if (this.onSurfaceReady) {
          this.onSurfaceReady(info);
        }
      })
      .onAreaChange((_oldValue, newValue): void => {
        this.viewWidth = Number(newValue.width);
        this.viewHeight = Number(newValue.height);
      })
      .onTouch((event: TouchEvent) => {
        if (this.mode !== 'control' || !this.onPointerEvent) {
          return;
        }
        const action = this.mapAction(event.action);
        if (!action) {
          return;
        }
        const xNorm = this.normalize(event.touch.windowX, this.viewWidth);
        const yNorm = this.normalize(event.touch.windowY, this.viewHeight);
        const payload: AgentSpacePointerEvent = {
          action,
          xNorm,
          yNorm,
          pointerId: event.touch.id,
        };
        this.onPointerEvent(payload);
      })
      .width('100%')
      .height('100%')
  }

  private resolveSurfaceSize(): { width: number; height: number } {
    if (this.surfaceWidth > 0 && this.surfaceHeight > 0) {
      return { width: this.surfaceWidth, height: this.surfaceHeight };
    }
    const d = display.getDefaultDisplaySync();
    return { width: d.width, height: d.height };
  }

  private normalize(value: number, limit: number): number {
    if (!Number.isFinite(value) || limit <= 0) {
      return 0;
    }
    const clamped = Math.min(Math.max(value / limit, 0), 1);
    return clamped;
  }

  private mapAction(action: Action): AgentSpacePointerAction | null {
    if (action === Action.DOWN) {
      return 'down';
    }
    if (action === Action.MOVE) {
      return 'move';
    }
    if (action === Action.UP) {
      return 'up';
    }
    if (action === Action.CANCEL) {
      return 'cancel';
    }
    return null;
  }
}
