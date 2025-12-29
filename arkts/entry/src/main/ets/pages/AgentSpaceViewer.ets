import { router } from '@kit.ArkUI';
import type { BusinessError } from '@kit.BasicServicesKit';
import { AgentSpace } from '../features/agentspace/AgentSpace';
import type { AgentSpacePointerEvent, AgentSpaceViewerEffect } from '../features/agentspace/AgentSpace';
import { GlowOverlay, GLOW_PALETTES } from '../ui_components/GlowOverlay';
import { FloatingOrb, FloatingOrbController } from '../ui_components/FloatingOrb';
import { StatusFloatWindow, StatusFloatWindowController } from '../ui_components/StatusFloatWindow';
import {
  InteractionEffectController,
  InteractionEffectOverlay,
  InteractionEffectOptions,
  InteractionPoint
} from '../ui_components/InteractionEffectOverlay';
import { AgentSpaceDisplay } from '../ui_components/AgentSpaceDisplay';
import type { AgentSpaceDisplayMode, AgentSpaceDisplaySurfaceInfo } from '../ui_components/AgentSpaceDisplay';

@Entry
@Component
struct AgentSpaceViewer {
  @State private viewerMode: AgentSpaceDisplayMode = 'display';
  @State @Watch('onDisplayEventSequence') private displayEventSequence: number = 0;
  @State private displaySurfaceInfo: AgentSpaceDisplaySurfaceInfo | null = null;
  @State private displayPointerEvent: AgentSpacePointerEvent | null = null;
  @State private interactionController: InteractionEffectController = InteractionEffectController.create();
  @State private showGlow: boolean = true;
  @State private showStatus: boolean = true;
  @State private statusController: StatusFloatWindowController = StatusFloatWindowController.create(
    'Viewer active',
    () => {},
    () => {},
    () => {},
    () => {
      this.switchToOrb();
    }
  );
  @State private orbController: FloatingOrbController = FloatingOrbController.create(
    'AgentSpace',
    'compact',
    () => {
      this.showFloatWindow();
    }
  );
  private agentSpace: AgentSpace | null = null;
  private effectSequence: number = 0;
  private effectPollTimerId: number = -1;
  private effectPolling: boolean = false;
  private readonly glowPaletteName: string = GLOW_PALETTES[0].name;

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

      GlowOverlay({ paletteName: this.glowPaletteName, visible: this.showGlow })
        .width('100%')
        .height('100%')

      InteractionEffectOverlay({ controller: this.interactionController })
        .width('100%')
        .height('100%')

      if (this.showStatus) {
        Stack()
          .width('100%')
          .height('100%')
          .backgroundColor('#00000000')
          .onClick(() => {
            this.switchToOrb();
          })
      }

      RelativeContainer() {
        StatusFloatWindow({ controller: this.statusController, visible: this.showStatus })
          .width('100%')
          .alignRules({
            bottom: { anchor: '__container__', align: VerticalAlign.Bottom },
            middle: { anchor: '__container__', align: HorizontalAlign.Center }
          })
      }
      .width('100%')
      .height('100%')

      FloatingOrb({ controller: this.orbController, visible: !this.showStatus })
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
    this.interactionController.configure({ coordScale: 1000 });
    this.startEffectPolling();
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
    this.stopEffectPolling();
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

  private startEffectPolling(): void {
    if (this.effectPollTimerId !== -1) {
      return;
    }
    this.effectSequence = 0;
    this.effectPollTimerId = setInterval(() => {
      void this.flushViewerEffects();
    }, 80);
  }

  private stopEffectPolling(): void {
    if (this.effectPollTimerId === -1) {
      return;
    }
    clearInterval(this.effectPollTimerId);
    this.effectPollTimerId = -1;
  }

  private async flushViewerEffects(): Promise<void> {
    if (this.effectPolling) {
      return;
    }
    this.effectPolling = true;
    try {
      const batch = await this.getAgentSpace().pullViewerEffects(this.effectSequence);
      if (!batch) {
        return;
      }
      this.effectSequence = batch.sequence;
      for (let i = 0; i < batch.effects.length; i++) {
        this.playEffect(batch.effects[i]);
      }
    } finally {
      this.effectPolling = false;
    }
  }

  private playEffect(effect: AgentSpaceViewerEffect): void {
    if (effect.kind === 'tap') {
      const point: InteractionPoint = { x: effect.xNorm, y: effect.yNorm };
      this.interactionController.tap(point);
      return;
    }
    if (effect.kind === 'long_press') {
      const point: InteractionPoint = { x: effect.xNorm, y: effect.yNorm };
      const options = this.resolveEffectOptions(effect.durationMs);
      this.interactionController.longPress(point, options);
      return;
    }
    if (effect.kind === 'swipe') {
      const start: InteractionPoint = { x: effect.startXNorm, y: effect.startYNorm };
      const end: InteractionPoint = { x: effect.endXNorm, y: effect.endYNorm };
      const options = this.resolveEffectOptions(effect.durationMs);
      this.interactionController.swipe(start, end, options);
    }
  }

  private resolveEffectOptions(durationMs?: number): InteractionEffectOptions | undefined {
    if (typeof durationMs !== 'number' || !Number.isFinite(durationMs)) {
      return undefined;
    }
    const payload: InteractionEffectOptions = { durationMs };
    return payload;
  }

  private switchToOrb(): void {
    this.showStatus = false;
  }

  private showFloatWindow(): void {
    this.showStatus = true;
  }
}
