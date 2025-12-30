import { router } from '@kit.ArkUI';
import type { BusinessError } from '@kit.BasicServicesKit';
import type { common } from '@kit.AbilityKit';
import type { AgentSpaceViewerEffect, AgentSpaceViewerEffectBatch, AgentSpaceViewerMode } from '../features/agentspace/AgentSpace';
import { AgentServiceClient } from '../features/agent_service/ipc/AgentServiceClient';
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
import type {
  AgentSpaceDisplayMode,
  AgentSpaceDisplaySurfaceInfo,
  AgentSpacePointerEvent
} from '../ui_components/AgentSpaceDisplay';

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
  @State private showOrb: boolean = false;
  @State private statusController: StatusFloatWindowController = StatusFloatWindowController.create(
    'Viewer active',
    () => {},
    () => {},
    () => {},
    () => {
      this.hideOverlays();
    }
  );
  @State private orbController: FloatingOrbController = FloatingOrbController.create(
    'AgentSpace',
    'compact',
    () => {
      this.showFloatWindow();
    }
  );
  private serviceClient: AgentServiceClient | null = null;
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
        Column() {
          Row()
            .layoutWeight(1)
            .width('100%')
            .backgroundColor('#00000001')
            .onClick(() => {
              this.switchToOrb();
            })

          StatusFloatWindow({ controller: this.statusController, visible: this.showStatus })
            .width('100%')
        }
        .width('100%')
        .height('100%')
        .padding({ left: 16, right: 16, bottom: 24 })
      }

      FloatingOrb({ controller: this.orbController, visible: this.showOrb })
    }
    .width('100%')
    .height('100%')
    .backgroundColor(Color.Black)
  }

  aboutToAppear(): void {
    void this.prepareViewer();
    this.interactionController.configure({ coordScale: 1000 });
    this.startEffectPolling();
  }

  onBackPress(): boolean | void {
    router.back();
    return true;
  }

  aboutToDisappear(): void {
    void this.updateViewerActive(false);
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
      void this.sendPointerEvent(pointerEvent);
    }
  }

  private async onSurfaceReady(info: AgentSpaceDisplaySurfaceInfo): Promise<void> {
    try {
      await this.ensureServiceConnected();
      await this.getServiceClient().attachViewerSurface(info.surfaceId);
    } catch (e) {
      const err = e as BusinessError;
      console.error(`[AgentSpaceViewer] attach surface failed. Code:${err.code}, message:${err.message}`);
    }
  }

  private getServiceClient(): AgentServiceClient {
    if (!this.serviceClient) {
      this.serviceClient = new AgentServiceClient();
    }
    return this.serviceClient;
  }

  private async ensureServiceConnected(): Promise<void> {
    const ctx = getContext(this) as common.UIAbilityContext;
    await this.getServiceClient().connect(ctx);
  }

  private async prepareViewer(): Promise<void> {
    try {
      await this.ensureServiceConnected();
      await this.getServiceClient().setViewerActive(true);
      const mode = await this.getServiceClient().getViewerMode();
      this.viewerMode = this.normalizeViewerMode(mode);
    } catch (e) {
      console.error(`[AgentSpaceViewer] service init failed: ${String(e)}`);
    }
  }

  private async updateViewerActive(active: boolean): Promise<void> {
    try {
      await this.ensureServiceConnected();
      await this.getServiceClient().setViewerActive(active);
    } catch (e) {
      console.error(`[AgentSpaceViewer] viewer active update failed: ${String(e)}`);
    }
  }

  private async sendPointerEvent(event: AgentSpacePointerEvent): Promise<void> {
    try {
      await this.ensureServiceConnected();
      const xNorm = this.normToInt(event.xNorm);
      const yNorm = this.normToInt(event.yNorm);
      const pointerId = this.toPointerId(event.pointerId);
      await this.getServiceClient().sendViewerEvent(event.action, xNorm, yNorm, pointerId);
    } catch (e) {
      console.error(`[AgentSpaceViewer] send pointer failed: ${String(e)}`);
    }
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
      await this.ensureServiceConnected();
      const payload = await this.getServiceClient().pullViewerEffects(this.effectSequence);
      const batch = this.parseEffectBatch(payload);
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

  private normalizeViewerMode(rawMode: string): AgentSpaceViewerMode {
    if (rawMode === 'control') {
      return 'control';
    }
    return 'display';
  }

  private parseEffectBatch(payload: string): AgentSpaceViewerEffectBatch | null {
    if (payload.length === 0) {
      return null;
    }
    try {
      const parsed = JSON.parse(payload) as AgentSpaceViewerEffectBatch;
      if (!parsed || typeof parsed.sequence !== 'number' || !Array.isArray(parsed.effects)) {
        return null;
      }
      return parsed;
    } catch (e) {
      console.error(`[AgentSpaceViewer] parse effects failed: ${String(e)}`);
      return null;
    }
  }

  private normToInt(value: number): number {
    if (!Number.isFinite(value)) {
      return 0;
    }
    const clamped = Math.min(Math.max(value, 0), 1);
    return Math.round(clamped * 1000);
  }

  private toPointerId(value?: number): number {
    if (typeof value !== 'number' || !Number.isFinite(value)) {
      return -1;
    }
    return Math.floor(value);
  }

  private switchToOrb(): void {
    this.showStatus = false;
    this.showGlow = true;
    this.showOrb = true;
    this.orbController.setMode('compact');
    this.orbController.show();
  }

  private showFloatWindow(): void {
    this.showStatus = true;
    this.showGlow = true;
    this.showOrb = false;
  }

  private hideOverlays(): void {
    this.showStatus = false;
    this.showGlow = false;
    this.showOrb = false;
  }
}
