import { router } from '@kit.ArkUI';
import type { BusinessError } from '@kit.BasicServicesKit';
import { AgentSpace } from '../features/agentspace/AgentSpace';
import type { AgentSpacePointerEvent } from '../features/agentspace/AgentSpace';
import { AgentSpaceDisplay } from '../ui_components/AgentSpaceDisplay';
import type { AgentSpaceDisplayMode, AgentSpaceDisplaySurfaceInfo } from '../ui_components/AgentSpaceDisplay';

@Entry
@Component
struct AgentSpaceViewer {
  @State private viewerMode: AgentSpaceDisplayMode = 'display';
  @State @Watch('onDisplayEventSequence') private displayEventSequence: number = 0;
  @State private displaySurfaceInfo: AgentSpaceDisplaySurfaceInfo | null = null;
  @State private displayPointerEvent: AgentSpacePointerEvent | null = null;
  private agentSpace: AgentSpace | null = null;

  build() {
    Stack() {
      AgentSpaceDisplay({
        mode: this.viewerMode,
        surfaceInfo: $displaySurfaceInfo,
        pointerEvent: $displayPointerEvent,
        eventSequence: $displayEventSequence,
      })
        .width('100%')
        .height('100%')
    }
    .width('100%')
    .height('100%')
    .backgroundColor(Color.Black)
  }

  aboutToAppear(): void {
    const space = AgentSpace.getInstance();
    this.agentSpace = space;
    void space.setViewerActive(true);
    void space.getViewerMode().then((mode) => {
      this.viewerMode = mode;
    });
  }

  onBackPress(): boolean | void {
    router.back();
    return true;
  }

  aboutToDisappear(): void {
    const agentSpace = AgentSpace.getInstance();
    void agentSpace.setViewerActive(false);
    this.displaySurfaceInfo = null;
    this.displayPointerEvent = null;
  }

  private onDisplayEventSequence(): void {
    const surfaceInfo = this.displaySurfaceInfo;
    if (surfaceInfo) {
      this.displaySurfaceInfo = null;
      void this.onSurfaceReady(surfaceInfo);
    }
    const pointerEvent = this.displayPointerEvent;
    if (pointerEvent) {
      this.displayPointerEvent = null;
      void this.getAgentSpace().sendViewerEvent(pointerEvent);
    }
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
    const space = AgentSpace.getInstance();
    this.agentSpace = space;
    return space;
  }
}
