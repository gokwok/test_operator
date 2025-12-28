import { display, router } from '@kit.ArkUI';
import type { BusinessError } from '@kit.BasicServicesKit';
import type { common } from '@kit.AbilityKit';
import { AgentSpace } from '../features/agentspace/AgentSpace';

interface SurfaceRect {
  surfaceWidth: number;
  surfaceHeight: number;
  offsetX: number;
}

@Entry
@Component
struct AgentSpaceViewer {
  private xComponentController: XComponentController = new XComponentController();
  private surfaceId: string = '';
  private surfaceWidth: number = -1;
  private surfaceHeight: number = -1;

  build() {
    Stack() {
      XComponent({
        type: XComponentType.SURFACE,
        controller: this.xComponentController
      })
        .onLoad(async () => {
          const ctx = getContext(this) as common.UIAbilityContext;
          const agentSpace = AgentSpace.getInstance(ctx);
          agentSpace.setViewerActive(true);
          await agentSpace.ensureVts();

          this.surfaceId = this.xComponentController.getXComponentSurfaceId();
          const rt = agentSpace.getRuntimeInfo();
          if (rt.widthPx > 0 && rt.heightPx > 0) {
            this.surfaceWidth = rt.widthPx;
            this.surfaceHeight = rt.heightPx;
          } else {
            const d = display.getDefaultDisplaySync();
            this.surfaceWidth = d.width;
            this.surfaceHeight = d.height;
          }

          const rect: SurfaceRect = {
            surfaceWidth: this.surfaceWidth,
            surfaceHeight: this.surfaceHeight,
            offsetX: 0,
          };
          this.xComponentController.setXComponentSurfaceRect(rect);
          try {
            await agentSpace.attachViewerSurface(this.surfaceId);
          } catch (e) {
            const err = e as BusinessError;
            console.error(`[AgentSpaceViewer] attach surface failed. Code:${err.code}, message:${err.message}`);
          }
        })
        .width('100%')
        .height('100%')
    }
    .width('100%')
    .height('100%')
    .backgroundColor(Color.Black)
  }

  onBackPress(): boolean | void {
    router.back();
    return true;
  }

  aboutToDisappear(): void {
    const ctx = getContext(this) as common.Context;
    const agentSpace = AgentSpace.getInstance(ctx);
    agentSpace.setViewerActive(false);
  }
}
