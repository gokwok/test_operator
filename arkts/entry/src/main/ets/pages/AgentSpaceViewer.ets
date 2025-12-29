import { router } from '@kit.ArkUI';
import type { BusinessError } from '@kit.BasicServicesKit';
import type { common } from '@kit.AbilityKit';
import { AgentSpace } from '../features/agentspace/AgentSpace';
import type { AgentSpacePointerEvent } from '../features/agentspace/AgentSpace';
import { AgentSpaceDisplay } from '../ui_components/AgentSpaceDisplay';
import type { AgentSpaceDisplayMode, AgentSpaceDisplaySurfaceInfo } from '../ui_components/AgentSpaceDisplay';

@Entry
@Component
struct AgentSpaceViewer {
  @State private viewerMode: AgentSpaceDisplayMode = 'display';
  private agentSpace: AgentSpace | null = null;
  private pointerHandler: ((event: AgentSpacePointerEvent) => void) | null = null;

  build() {
    Stack() {
      AgentSpaceDisplay({
        mode: this.viewerMode,
        onSurfaceReady: (info: AgentSpaceDisplaySurfaceInfo) => {
          void this.onSurfaceReady(info);
        },
        onPointerEvent: this.pointerHandler ?? undefined,
      })
        .width('100%')
        .height('100%')
    }
    .width('100%')
    .height('100%')
    .backgroundColor(Color.Black)
  }

  aboutToAppear(): void {
    const ctx = getContext(this) as common.UIAbilityContext;
    const space = AgentSpace.getInstance(ctx);
    this.agentSpace = space;
    space.setViewerActive(true);
    this.viewerMode = space.getViewerMode();
    this.pointerHandler = space.handleViewerPointer.bind(space);
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

  private async onSurfaceReady(info: AgentSpaceDisplaySurfaceInfo): Promise<void> {
    const space = this.getAgentSpace();
    try {
      await space.ensureVts();
      await space.attachViewerSurface(info.surfaceId);
    } catch (e) {
      const err = e as BusinessError;
      console.error(`[AgentSpaceViewer] attach surface failed. Code:${err.code}, message:${err.message}`);
    }
  }

  private getAgentSpace(): AgentSpace {
    if (this.agentSpace) {
      return this.agentSpace;
    }
    const ctx = getContext(this) as common.UIAbilityContext;
    const space = AgentSpace.getInstance(ctx);
    this.agentSpace = space;
    return space;
  }
}
