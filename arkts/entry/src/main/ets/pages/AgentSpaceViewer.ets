import { router } from '@kit.ArkUI';
import type { BusinessError } from '@kit.BasicServicesKit';
import type { common } from '@kit.AbilityKit';
import type { AgentSpaceViewerEffect, AgentSpaceViewerEffectBatch, AgentSpaceViewerMode } from '../features/agentspace/AgentSpace';
import { AgentServiceClient } from '../features/agent_service/ipc/AgentServiceClient';
import type { TaskSnapshot, UserReply } from '../features/agent_service/types';
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

interface ParsedInteractionParams {
  content?: string;
  options?: Array<string>;
}

interface ParsedInteractionAction {
  name?: string;
  parameters?: ParsedInteractionParams;
}

interface InteractionPayload {
  kind: 'call_user' | 'interact';
  content: string;
  options: Array<string>;
}

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
    () => {
      void this.onStopRequested();
    },
    () => {
      void this.onCollapseRequested();
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
  private statusPollTimerId: number = -1;
  private statusPolling: boolean = false;
  private lastStatusText: string = '';
  private callUserActive: boolean = false;
  private callUserContent: string = '';
  private callUserKind: 'call_user' | 'interact' = 'call_user';
  private readonly defaultGlowPaletteName: string = GLOW_PALETTES[0].name;
  @State private glowPaletteName: string = GLOW_PALETTES[0].name;

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
    this.configureStatusController();
    this.startEffectPolling();
    this.startStatusPolling();
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
    this.stopStatusPolling();
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

  private startStatusPolling(): void {
    if (this.statusPollTimerId !== -1) {
      return;
    }
    this.statusPollTimerId = setInterval(() => {
      void this.flushTaskStatus();
    }, 500);
  }

  private stopStatusPolling(): void {
    if (this.statusPollTimerId === -1) {
      return;
    }
    clearInterval(this.statusPollTimerId);
    this.statusPollTimerId = -1;
  }

  private async flushTaskStatus(): Promise<void> {
    if (this.statusPolling) {
      return;
    }
    this.statusPolling = true;
    try {
      await this.ensureServiceConnected();
      const snapshotText = await this.getServiceClient().getSnapshot();
      const snapshot = this.readSnapshot(snapshotText);
      if (!snapshot) {
        return;
      }
      const interaction = this.extractInteraction(snapshot);
      if (interaction) {
        if (!this.callUserActive ||
          interaction.content !== this.callUserContent ||
          interaction.kind !== this.callUserKind) {
          this.enterInteraction(interaction);
        }
        return;
      }
      if (this.callUserActive) {
        if (snapshot.status !== 'waiting_user') {
          this.exitCallUser();
        } else {
          return;
        }
      }

      if (snapshot.status === 'finished') {
        this.enterTaskFinished(snapshot);
        return;
      }

      if (this.glowPaletteName !== this.defaultGlowPaletteName) {
        this.glowPaletteName = this.defaultGlowPaletteName;
      }

      const description = snapshot.lastDescription.trim();
      const resolved = description.length > 0 ? description : '正在分析用户需求...';
      if (this.isUserReplyStatusLocked(snapshot)) {
        return;
      }
      if (resolved !== this.lastStatusText) {
        this.lastStatusText = resolved;
        this.statusController.setStatus(resolved);
      }
    } catch (e) {
      console.error(`[AgentSpaceViewer] status poll failed: ${String(e)}`);
    } finally {
      this.statusPolling = false;
    }
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

  private readSnapshot(payload: string): TaskSnapshot | null {
    if (payload.length === 0) {
      return null;
    }
    try {
      const parsed = JSON.parse(payload) as TaskSnapshot;
      if (!parsed || typeof parsed.lastDescription !== 'string' || typeof parsed.status !== 'string') {
        return null;
      }
      return parsed;
    } catch (e) {
      console.error(`[AgentSpaceViewer] parse snapshot failed: ${String(e)}`);
      return null;
    }
  }

  private extractInteraction(snapshot: TaskSnapshot): InteractionPayload | null {
    if (snapshot.status !== 'waiting_user') {
      return null;
    }
    const actionJson = snapshot.lastActionJson ?? '';
    if (actionJson.length === 0) {
      return null;
    }
    try {
      const action = JSON.parse(actionJson) as ParsedInteractionAction;
      const name = action?.name?.toLowerCase() ?? '';
      if (name !== 'call_user' && name !== 'interact') {
        return null;
      }
      const content = action?.parameters?.content ?? '';
      const normalizedContent = typeof content === 'string' ? content.trim() : '';
      if (normalizedContent.length === 0) {
        return null;
      }
      const options = this.normalizeOptions(action?.parameters?.options);
      const payload: InteractionPayload = {
        kind: name === 'interact' ? 'interact' : 'call_user',
        content: normalizedContent,
        options,
      };
      return payload;
    } catch {
      return null;
    }
  }

  private normalizeOptions(raw?: Array<string>): Array<string> {
    if (!raw || raw.length === 0) {
      return [];
    }
    const result: Array<string> = [];
    for (let i = 0; i < raw.length; i++) {
      const value = raw[i];
      if (typeof value !== 'string') {
        continue;
      }
      const trimmed = value.trim();
      if (trimmed.length === 0) {
        continue;
      }
      result.push(trimmed);
    }
    return result;
  }

  private isUserReplyStatusLocked(snapshot: TaskSnapshot): boolean {
    if (!this.lastStatusText.startsWith('已收到你的输入信息')) {
      return false;
    }
    return snapshot.lastActionName === 'call_user' || snapshot.lastActionName === 'interact';
  }

  private configureStatusController(): void {
    this.statusController.setCallUserActionLabel('告诉AIOS');
    this.statusController.setCallUserConfirmHandler(() => {
      this.enterCallUserInput();
    });
    this.statusController.setCallUserBackHandler(() => {
      this.enterCallUserWait();
    });
    this.statusController.setCallUserSendHandler((text: string) => {
      void this.submitCallUserReply(text);
    });
    this.statusController.setCallUserOptionHandler((label: string) => {
      this.onInteractionOption(label);
    });
  }

  private enterInteraction(payload: InteractionPayload): void {
    this.callUserActive = true;
    this.callUserKind = payload.kind;
    this.callUserContent = payload.content;
    this.statusController.setStopped(false);
    this.statusController.setCallUserContent(payload.content);
    this.statusController.clearCallUserInput();
    if (payload.kind === 'interact') {
      this.statusController.setCallUserOptions(payload.options);
    } else {
      this.statusController.setCallUserOptions([]);
    }
    this.statusController.setMode('call_user_wait');
    this.showFloatWindow();
  }

  private exitCallUser(): void {
    this.callUserActive = false;
    this.callUserContent = '';
    this.callUserKind = 'call_user';
    this.statusController.clearCallUserInput();
    this.statusController.setCallUserOptions([]);
    this.statusController.setMode('default');
  }

  private enterTaskFinished(snapshot: TaskSnapshot): void {
    if (this.callUserActive) {
      this.exitCallUser();
    }
    const content = this.extractFinishContent(snapshot);
    this.statusController.setMode('default');
    this.statusController.setTerminal('finished', content);
    this.lastStatusText = content;
    this.statusController.setStatus(content);
    this.showFloatWindow();
    this.showGlow = true;
    this.glowPaletteName = 'Lagoon';
  }

  private extractFinishContent(snapshot: TaskSnapshot): string {
    const actionJson = snapshot.lastActionJson ?? '';
    if (actionJson.length > 0) {
      try {
        const action = JSON.parse(actionJson) as ParsedInteractionAction;
        const name = action?.name?.toLowerCase() ?? '';
        if (name === 'finish') {
          const content = action?.parameters?.content ?? '';
          const normalized = typeof content === 'string' ? content.trim() : '';
          if (normalized.length > 0) {
            return normalized;
          }
        }
      } catch (_e) {
        // ignore
      }
    }
    const description = snapshot.lastDescription.trim();
    if (description.length > 0) {
      return description;
    }
    return '任务已完成';
  }

  private async onStopRequested(): Promise<void> {
    if (this.callUserActive) {
      this.exitCallUser();
    }
    try {
      await this.ensureServiceConnected();
      await this.getServiceClient().stopTask('user_stop');
    } catch (e) {
      console.error(`[AgentSpaceViewer] stop task failed: ${String(e)}`);
    }
  }

  private async onCollapseRequested(): Promise<void> {
    this.hideOverlays();
    try {
      await this.ensureServiceConnected();
      await this.getServiceClient().transferToMain();
    } catch (e) {
      console.error(`[AgentSpaceViewer] transfer to main failed: ${String(e)}`);
    }
    router.back();
  }

  private enterCallUserWait(): void {
    if (!this.callUserActive) {
      return;
    }
    this.statusController.setMode('call_user_wait');
  }

  private enterCallUserInput(): void {
    if (!this.callUserActive) {
      return;
    }
    this.statusController.setMode('call_user_input');
  }

  private async submitCallUserReply(text: string): Promise<void> {
    await this.submitUserReply('call_user', text);
  }

  private onInteractionOption(label: string): void {
    if (this.callUserKind === 'call_user') {
      this.enterCallUserInput();
      return;
    }
    void this.submitUserReply('interact', label);
  }

  private async submitUserReply(kind: 'call_user' | 'interact', text: string): Promise<void> {
    const trimmed = text.trim();
    if (trimmed.length === 0) {
      return;
    }
    try {
      await this.ensureServiceConnected();
      const reply: UserReply = { kind, content: trimmed };
      await this.getServiceClient().submitUserReply(reply);
      this.callUserActive = false;
      this.callUserContent = '';
      this.callUserKind = 'call_user';
      this.statusController.clearCallUserInput();
      this.statusController.setCallUserOptions([]);
      this.statusController.setMode('default');
      const message = `已收到你的输入信息：\n「${trimmed}」`;
      this.lastStatusText = message;
      this.statusController.setStatus(message);
      this.showFloatWindow();
    } catch (e) {
      console.error(`[AgentSpaceViewer] submit user reply failed: ${String(e)}`);
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
    if (this.callUserActive) {
      return;
    }
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
