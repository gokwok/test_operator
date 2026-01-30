import { router } from '@kit.ArkUI';
import type { BusinessError } from '@kit.BasicServicesKit';
import type { common } from '@kit.AbilityKit';
import image from '@ohos.multimedia.image';
import type { AgentSpaceViewerEffect, AgentSpaceViewerEffectBatch, AgentSpaceViewerMode } from '../../features/agent_controller/agentspace/AgentSpace';
import { AgentControllerClient } from '../../features/agent_controller/ipc/AgentControllerClient';
import type { TaskEvent, TaskEventBatch, TaskSnapshot, UserReply } from '../../features/agent_controller/types';
import { WallpaperState } from '../../features/agent_controller/state_collector/WallpaperState';
import { GlowOverlay, GLOW_PALETTES } from '../views/GlowOverlay';
import { FloatingOrb, FloatingOrbController } from '../views/FloatingOrb';
import { StatusFloatWindow, StatusFloatWindowController } from '../views/StatusFloatWindow';
import {
  InteractionEffectController,
  InteractionEffectOverlay,
  InteractionEffectOptions,
  InteractionPoint
} from '../views/InteractionEffectOverlay';
import { AgentSpaceDisplay } from '../views/AgentSpaceDisplay';
import type {
  AgentSpaceDisplayMode,
  AgentSpaceDisplaySurfaceInfo,
  AgentSpacePointerEvent
} from '../views/AgentSpaceDisplay';

const EFFECT_LOG_TAG: string = '[AS_EFFECT]';

interface ParsedInteractionParams {
  content?: string;
  options?: Array<string>;
}

interface ParsedInteractionAction {
  name?: string;
  parameters?: ParsedInteractionParams;
}

interface InteractionPayload {
  kind: 'call_user' | 'interact' | 'take_over';
  content: string;
  options: Array<string>;
}

interface A2AStatusMessagePart {
  text?: string;
}

interface A2AStatusMessageMetadata {
  action?: ParsedInteractionAction;
  options?: Array<string>;
}

interface A2AStatusMessage {
  parts?: Array<A2AStatusMessagePart>;
  metadata?: A2AStatusMessageMetadata;
}

interface A2AStatusUpdate {
  state?: string;
  message?: A2AStatusMessage;
}

interface A2AArtifactPart {
  kind?: string;
  data?: Object;
  text?: string;
}

interface A2AArtifact {
  artifactId?: string;
  parts?: Array<A2AArtifactPart>;
}

interface A2AArtifactUpdate {
  artifact?: A2AArtifact;
  artifacts?: Array<A2AArtifact>;
}

interface A2AStreamUpdate {
  statusUpdate?: A2AStatusUpdate;
  artifactUpdate?: A2AArtifactUpdate;
  artifacts?: Array<A2AArtifact>;
}

interface A2ADeltaData {
  kind?: string;
  description?: string;
}

interface A2AOutputMessagePart {
  kind?: string;
  text?: string;
}

interface A2AOutputMessage {
  parts?: Array<A2AOutputMessagePart>;
}

interface A2AOutputActionParameters {
  content?: string;
  message?: string;
}

interface A2AOutputAction {
  name?: string;
  parameters?: A2AOutputActionParameters;
}

interface A2AOutputData {
  result?: string;
  session_id?: string;
  description?: string;
  text?: string;
  answer?: string;
  message?: string | A2AOutputMessage;
  action?: A2AOutputAction;
  output?: A2AOutputData;
  final?: A2AOutputData;
}

interface A2AToolTracePayload {
  tool_name?: string;
  args?: Object;
}

interface A2AToolTraceRecord {
  type?: string;
  payload?: A2AToolTracePayload;
}

interface A2AToolTraceArgs {
  redacted?: boolean;
  point?: Array<number>;
  start?: Array<number>;
  start_point?: Array<number>;
  end?: Array<number>;
  end_point?: Array<number>;
  duration_ms?: number;
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
  @State private wallpaper: image.PixelMap | null = null;
  @State private showDisplay: boolean = false;
  @State private manualControlActive: boolean = false;
  @State private statusController: StatusFloatWindowController = StatusFloatWindowController.create(
    '',
    () => {
      void this.onSupplementRequested();
    },
    () => {
      void this.onTakeoverRequested();
    },
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
      this.onOrbClick();
    }
  );
  private serviceClient: AgentControllerClient | null = null;
  private effectSequence: number = 0;
  private effectPollTimerId: number = -1;
  private effectPolling: boolean = false;
  private statusPollTimerId: number = -1;
  private statusPolling: boolean = false;
  private a2aEventSequence: number = 0;
  private a2aEventMode: boolean = false;
  private a2aLastState: string = '';
  private lastStatusText: string = '';
  private lastFinalOutputText: string = '';
  private assistantTextBuffer: string = '';
  private hasFinalOutput: boolean = false;
  private callUserActive: boolean = false;
  private callUserContent: string = '';
  private callUserKind: 'call_user' | 'interact' | 'take_over' = 'call_user';
  private supplementActive: boolean = false;
  private takeOverIdleTimerId: number = -1;
  private takeOverCountdownTimerId: number = -1;
  private takeOverCountdownValue: number = 0;
  private takeOverPreviousMode: AgentSpaceDisplayMode = 'display';
  private readonly takeOverIdleMs: number = 5000;
  private readonly takeOverCountdownStart: number = 5;
  private readonly orbDefaultText: string = 'AgentSpace';
  private readonly manualTakeOverRequestContent: string = '用户主动接管操作';
  private readonly manualTakeOverReply: string = '用户刚刚进行了操作，现在我将继续为用户执行任务';
  private readonly supplementRequestContent: string = '请补充必要信息';
  private readonly defaultGlowPaletteName: string = GLOW_PALETTES[0].name;
  private readonly effectBaseColor: string = '#FF4D5B';
  private readonly effectStrength: number = 1.1;
  @State private glowPaletteName: string = GLOW_PALETTES[0].name;

  build() {
    Stack() {
      if (this.wallpaper) {
        Image(this.wallpaper)
          .width('100%')
          .height('100%')
          .objectFit(ImageFit.Cover)
      }
      if (this.showDisplay) {
        AgentSpaceDisplay({
          mode: this.viewerMode,
          surfaceInfo: $displaySurfaceInfo,
          pointerEvent: $displayPointerEvent,
          eventSequence: $displayEventSequence,
        })
          .width('100%')
          .height('100%')
      }

      if (!this.manualControlActive && this.showGlow) {
        GlowOverlay({ paletteName: this.glowPaletteName, visible: this.showGlow })
          .width('100%')
          .height('100%')
          .hitTestBehavior(HitTestMode.Transparent)
      }

      if (!this.manualControlActive) {
        InteractionEffectOverlay({ controller: this.interactionController, baseColor: this.effectBaseColor })
          .width('100%')
          .height('100%')
          .hitTestBehavior(HitTestMode.Transparent)
      }

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
    this.a2aEventSequence = 0;
    this.a2aEventMode = false;
    void this.prepareViewer();
    void this.loadWallpaper();
    this.interactionController.configure({
      coordScale: 1000,
      defaults: {
        tap: { color: this.effectBaseColor, strength: this.effectStrength },
        longPress: { color: this.effectBaseColor, strength: this.effectStrength },
        swipe: { color: this.effectBaseColor, strength: this.effectStrength },
      }
    });
    this.configureStatusController();
    this.startEffectPolling();
    this.startStatusPolling();
  }

  onBackPress(): boolean | void {
    void this.onCollapseRequested();
    return true;
  }

  aboutToDisappear(): void {
    void this.updateViewerActive(false);
    this.displaySurfaceInfo = null;
    this.displayPointerEvent = null;
    this.stopManualControl();
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
      if (this.manualControlActive) {
        this.recordManualControlActivity();
      }
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

  private getServiceClient(): AgentControllerClient {
    if (!this.serviceClient) {
      this.serviceClient = new AgentControllerClient();
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
      console.info(`${EFFECT_LOG_TAG} viewer active set true`);
      const mode = await this.getServiceClient().getViewerMode();
      this.viewerMode = this.normalizeViewerMode(mode);
    } catch (e) {
      console.error(`[AgentSpaceViewer] service init failed: ${String(e)}`);
    }
  }

  private async loadWallpaper(): Promise<void> {
    this.wallpaper = await WallpaperState.getCurrentWallpaper();
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
    console.info(`${EFFECT_LOG_TAG} start effect polling`);
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
    console.info(`${EFFECT_LOG_TAG} stop effect polling`);
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
      const gotEvents = await this.flushA2AEvents();
      if (this.a2aEventMode || gotEvents) {
        await this.refreshDisplayVisibility();
        return;
      }

      const snapshotText = await this.getServiceClient().getSnapshot();
      const snapshot = this.readSnapshot(snapshotText);
      if (!snapshot) {
        return;
      }
      this.updateDisplayVisibility(snapshot);
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

  private async refreshDisplayVisibility(): Promise<void> {
    try {
      const snapshotText = await this.getServiceClient().getSnapshot();
      const snapshot = this.readSnapshot(snapshotText);
      if (snapshot) {
        this.updateDisplayVisibility(snapshot);
      }
    } catch (_e) {
      // ignore display refresh failures
    }
  }

  private async flushA2AEvents(): Promise<boolean> {
    const payload = await this.getServiceClient().a2aPullEvents(this.a2aEventSequence);
    const batch = this.parseA2AEventBatch(payload);
    if (!batch) {
      return false;
    }
    this.a2aEventSequence = batch.sequence;
    if (!this.a2aEventMode) {
      this.a2aEventMode = true;
      this.stopEffectPolling();
    }
    for (let i = 0; i < batch.events.length; i += 1) {
      this.handleA2AEvent(batch.events[i]);
    }
    return batch.events.length > 0;
  }

  private parseA2AEventBatch(payload: string): TaskEventBatch | null {
    if (payload.length === 0) {
      return null;
    }
    try {
      const parsed = JSON.parse(payload) as TaskEventBatch;
      if (!parsed || typeof parsed.sequence !== 'number' || !Array.isArray(parsed.events)) {
        return null;
      }
      return parsed;
    } catch (e) {
      console.error(`[AgentSpaceViewer] parse A2A event batch failed: ${String(e)}`);
      return null;
    }
  }

  private handleA2AEvent(event: TaskEvent): void {
    if (event.type === 'assistant.delta' || event.type === 'delta') {
      const update = this.parseA2AUpdate(event.payloadJson);
      if (!update) {
        return;
      }
      const finalText = this.extractFinalOutput(update);
      if (finalText.length > 0) {
        this.lastFinalOutputText = finalText;
        this.hasFinalOutput = true;
        this.applyFinalOutputIfFinished(finalText);
        return;
      }
      const chunk = this.extractAssistantTextChunk(update);
      if (chunk.length > 0) {
        this.assistantTextBuffer += chunk;
        if (!this.hasFinalOutput) {
          this.lastFinalOutputText = this.assistantTextBuffer.trim();
          this.applyFinalOutputIfFinished(this.lastFinalOutputText);
        }
        return;
      }
      const description = this.extractDescription(update);
      if (description.length > 0) {
        this.updateStatusText(description);
      }
      return;
    }
    if (event.type === 'status' || event.type === 'input_required') {
      const update = this.parseA2AUpdate(event.payloadJson);
      if (update) {
        this.handleA2AStatusUpdate(update, event.message);
      } else if (event.message.length > 0) {
        this.updateStatusText(event.message);
      }
      return;
    }
    if (event.type === 'tool.trace') {
      const update = this.parseA2AUpdate(event.payloadJson);
      if (update) {
        this.handleToolTrace(update);
      }
      return;
    }
    if (event.type === 'error') {
      if (event.message.length > 0) {
        this.updateStatusText(event.message);
      }
    }
  }

  private parseA2AUpdate(payloadJson: string): A2AStreamUpdate | null {
    if (payloadJson.length === 0) {
      return null;
    }
    try {
      const parsed = JSON.parse(payloadJson) as A2AStreamUpdate;
      return parsed ?? null;
    } catch (e) {
      console.error(`[AgentSpaceViewer] parse A2A update failed: ${String(e)}`);
      return null;
    }
  }

  private handleA2AStatusUpdate(update: A2AStreamUpdate, fallbackMessage: string): void {
    const statusUpdate = update.statusUpdate;
    if (!statusUpdate) {
      if (fallbackMessage.length > 0) {
        this.updateStatusText(fallbackMessage);
      }
      return;
    }
    const state = typeof statusUpdate.state === 'string' ? statusUpdate.state : '';
    if (state === 'working' && this.a2aLastState !== 'working') {
      this.lastFinalOutputText = '';
      this.assistantTextBuffer = '';
      this.hasFinalOutput = false;
    }
    if (state.length > 0) {
      this.a2aLastState = state;
    }
    const messageText = this.readStatusMessage(statusUpdate);
    if (state === 'input-required') {
      const interaction = this.extractInteractionFromA2A(statusUpdate, messageText);
      if (interaction) {
        this.enterInteraction(interaction);
        return;
      }
      if (messageText.length > 0) {
        this.updateStatusText(messageText);
      }
      return;
    }
    if (this.callUserActive) {
      this.exitCallUser();
    }
    if (state === 'working') {
      if (messageText.length > 0) {
        this.updateStatusText(messageText);
      }
      return;
    }
    if (state === 'completed') {
      this.enterA2ATaskFinished(messageText);
      return;
    }
    if (state === 'failed') {
      this.enterA2ATaskFailed(messageText);
      return;
    }
    if (messageText.length > 0) {
      this.updateStatusText(messageText);
      return;
    }
    if (fallbackMessage.length > 0) {
      this.updateStatusText(fallbackMessage);
    }
  }

  private readStatusMessage(statusUpdate: A2AStatusUpdate): string {
    const message = statusUpdate.message;
    if (!message) {
      return '';
    }
    const parts = message.parts;
    if (!Array.isArray(parts) || parts.length === 0) {
      return '';
    }
    const first = parts[0];
    const text = first ? first.text : undefined;
    if (typeof text === 'string') {
      return text.trim();
    }
    return '';
  }

  private extractInteractionFromA2A(statusUpdate: A2AStatusUpdate, messageText: string): InteractionPayload | null {
    const message = statusUpdate.message;
    const metadata = message?.metadata;
    const action = metadata?.action;
    if (!action || typeof action !== 'object') {
      return null;
    }
    const rawName = action.name ?? '';
    const name = typeof rawName === 'string' ? rawName.trim().toLowerCase() : '';
    if (name !== 'call_user' && name !== 'interact' && name !== 'take_over') {
      return null;
    }
    const params: ParsedInteractionParams = action.parameters ?? {};
    const contentRaw = typeof params.content === 'string' ? params.content : messageText;
    const content = typeof contentRaw === 'string' ? contentRaw.trim() : '';
    if (content.length === 0) {
      return null;
    }
    const optionsRaw = metadata?.options ?? params.options;
    const options = this.normalizeOptions(Array.isArray(optionsRaw) ? optionsRaw : undefined);
    const payload: InteractionPayload = {
      kind: name === 'interact' ? 'interact' : name === 'take_over' ? 'take_over' : 'call_user',
      content,
      options,
    };
    return payload;
  }

  private extractDescription(update: A2AStreamUpdate): string {
    const artifactUpdate = update.artifactUpdate;
    const artifact = artifactUpdate?.artifact;
    const parts = artifact?.parts;
    if (!Array.isArray(parts)) {
      return '';
    }
    for (let i = 0; i < parts.length; i += 1) {
      const part = parts[i];
      if (!part) {
        continue;
      }
      if (part.kind === 'text') {
        const text = part.text;
        if (typeof text === 'string' && text.trim().length > 0) {
          return text.trim();
        }
        continue;
      }
      if (part.kind !== 'data') {
        continue;
      }
      const delta = part.data as A2ADeltaData;
      if (delta && (delta.kind === 'agent.description' || typeof delta.kind !== 'string')) {
        const desc = delta.description;
        if (typeof desc === 'string' && desc.trim().length > 0) {
          return desc.trim();
        }
      }
      const output = part.data as A2AOutputData;
      if (output) {
        const candidates = [output.description, output.text, output.answer];
        for (let j = 0; j < candidates.length; j += 1) {
          const value = candidates[j];
          if (typeof value === 'string' && value.trim().length > 0) {
            return value.trim();
          }
        }
      }
    }
    return '';
  }

  private readA2AArtifactId(update: A2AStreamUpdate): string {
    const artifactUpdate = update.artifactUpdate;
    const artifact = artifactUpdate?.artifact;
    if (artifact && typeof artifact.artifactId === 'string' && artifact.artifactId.length > 0) {
      return artifact.artifactId;
    }
    const artifacts = artifactUpdate?.artifacts;
    if (Array.isArray(artifacts)) {
      for (let i = 0; i < artifacts.length; i += 1) {
        const item = artifacts[i];
        if (item && typeof item.artifactId === 'string' && item.artifactId.length > 0) {
          return item.artifactId;
        }
      }
    }
    const fallback = update.artifacts;
    if (Array.isArray(fallback)) {
      for (let i = 0; i < fallback.length; i += 1) {
        const item = fallback[i];
        if (item && typeof item.artifactId === 'string' && item.artifactId.length > 0) {
          return item.artifactId;
        }
      }
    }
    return '';
  }

  private collectA2AArtifacts(update: A2AStreamUpdate): Array<A2AArtifact> {
    const artifacts: Array<A2AArtifact> = [];
    const artifactUpdate = update.artifactUpdate;
    const single = artifactUpdate?.artifact;
    if (single) {
      artifacts.push(single);
    }
    const list = artifactUpdate?.artifacts;
    if (Array.isArray(list)) {
      for (let i = 0; i < list.length; i += 1) {
        const item = list[i];
        if (item) {
          artifacts.push(item);
        }
      }
    }
    const fallback = update.artifacts;
    if (Array.isArray(fallback)) {
      for (let i = 0; i < fallback.length; i += 1) {
        const item = fallback[i];
        if (item) {
          artifacts.push(item);
        }
      }
    }
    return artifacts;
  }

  private extractAssistantTextChunk(update: A2AStreamUpdate): string {
    const artifacts = this.collectA2AArtifacts(update);
    for (let i = 0; i < artifacts.length; i += 1) {
      const artifact = artifacts[i];
      if (!artifact || artifact.artifactId !== 'assistant.text') {
        continue;
      }
      const parts = artifact.parts;
      if (!Array.isArray(parts)) {
        continue;
      }
      for (let j = 0; j < parts.length; j += 1) {
        const part = parts[j];
        if (!part || part.kind !== 'text') {
          continue;
        }
        const text = part.text;
        if (typeof text === 'string' && text.length > 0) {
          return text;
        }
      }
    }
    return '';
  }

  private extractFinalOutput(update: A2AStreamUpdate): string {
    const artifacts = this.collectA2AArtifacts(update);
    for (let i = 0; i < artifacts.length; i += 1) {
      const artifact = artifacts[i];
      if (!artifact || artifact.artifactId !== 'thinkflow.output') {
        continue;
      }
      const parts = artifact.parts;
      if (!Array.isArray(parts)) {
        continue;
      }
      for (let j = 0; j < parts.length; j += 1) {
        const part = parts[j];
        if (!part) {
          continue;
        }
        if (part.kind === 'text') {
          const text = part.text;
          if (typeof text === 'string' && text.trim().length > 0) {
            return text.trim();
          }
          continue;
        }
        if (part.kind !== 'data') {
          continue;
        }
        const output = part.data as A2AOutputData;
        const text = this.readOutputText(output);
        if (text.length > 0) {
          return text;
        }
      }
    }
    return '';
  }

  private readOutputText(output?: A2AOutputData): string {
    if (!output) {
      return '';
    }
    const actionText = this.readOutputActionContent(output.action);
    if (actionText.length > 0) {
      return actionText;
    }
    if (typeof output.text === 'string' && output.text.trim().length > 0) {
      return output.text.trim();
    }
    if (typeof output.answer === 'string' && output.answer.trim().length > 0) {
      return output.answer.trim();
    }
    const messageRaw = output.message;
    if (typeof messageRaw === 'string' && messageRaw.trim().length > 0) {
      return messageRaw.trim();
    }
    if (messageRaw && typeof messageRaw === 'object' && !Array.isArray(messageRaw)) {
      const messageText = this.readOutputMessageText(messageRaw as A2AOutputMessage);
      if (messageText.length > 0) {
        return messageText;
      }
    }
    if (typeof output.description === 'string' && output.description.trim().length > 0) {
      return output.description.trim();
    }
    const nestedFinal = output.final;
    if (nestedFinal && typeof nestedFinal === 'object' && !Array.isArray(nestedFinal)) {
      return this.readOutputText(nestedFinal);
    }
    const nested = output.output;
    if (nested && typeof nested === 'object' && !Array.isArray(nested)) {
      return this.readOutputText(nested);
    }
    return '';
  }

  private readOutputActionContent(action?: A2AOutputAction): string {
    if (!action) {
      return '';
    }
    const nameRaw = action.name;
    const name = typeof nameRaw === 'string' ? nameRaw.trim().toLowerCase() : '';
    if (name !== 'finish') {
      return '';
    }
    const params = action.parameters;
    const contentRaw = params?.content ?? params?.message;
    if (typeof contentRaw !== 'string') {
      return '';
    }
    const content = contentRaw.trim();
    return content.length > 0 ? content : '';
  }

  private readOutputMessageText(message?: A2AOutputMessage): string {
    const parts = message?.parts;
    if (!Array.isArray(parts)) {
      return '';
    }
    for (let i = 0; i < parts.length; i += 1) {
      const part = parts[i];
      if (!part || part.kind !== 'text') {
        continue;
      }
      const text = part.text;
      if (typeof text === 'string' && text.trim().length > 0) {
        return text.trim();
      }
    }
    return '';
  }

  private updateStatusText(text: string): void {
    if (this.callUserActive) {
      return;
    }
    const resolved = text.length > 0 ? text : '正在分析用户需求...';
    if (resolved === this.lastStatusText) {
      return;
    }
    this.lastStatusText = resolved;
    this.statusController.setStatus(resolved);
  }

  private enterA2ATaskFinished(message: string): void {
    if (this.callUserActive) {
      this.exitCallUser();
    }
    const fallback = this.lastFinalOutputText.length > 0 ? this.lastFinalOutputText : '任务已完成';
    const content = message.length > 0 ? message : fallback;
    this.statusController.setMode('default');
    this.statusController.setTerminal('finished', content);
    this.lastStatusText = content;
    this.statusController.setStatus(content);
    this.showFloatWindow();
    this.showGlow = true;
    this.glowPaletteName = 'Lagoon';
  }

  private enterA2ATaskFailed(message: string): void {
    if (this.callUserActive) {
      this.exitCallUser();
    }
    const content = message.length > 0 ? message : '任务失败';
    this.statusController.setMode('default');
    this.statusController.setTerminal('stopped', content);
    this.lastStatusText = content;
    this.statusController.setStatus(content);
    this.showFloatWindow();
  }

  private applyFinalOutputIfFinished(content: string): void {
    if (!this.statusController.stopped) {
      return;
    }
    if (this.statusController.terminalKind !== 'finished') {
      return;
    }
    const normalized = content.trim();
    if (normalized.length === 0) {
      return;
    }
    const current = this.statusController.terminalContent.trim();
    if (current.length > 0 && current !== '任务已完成') {
      return;
    }
    this.statusController.setTerminal('finished', normalized);
    this.lastStatusText = normalized;
    this.statusController.setStatus(normalized);
  }

  private handleToolTrace(update: A2AStreamUpdate): void {
    const record = this.readToolTraceRecord(update);
    if (!record || typeof record.type !== 'string') {
      return;
    }
    if (!record.type.startsWith('tool.call.started')) {
      return;
    }
    const payload = record.payload;
    const toolNameRaw = payload?.tool_name;
    const toolName = typeof toolNameRaw === 'string' ? toolNameRaw.trim().toLowerCase() : '';
    if (toolName.length === 0) {
      return;
    }
    const args = payload?.args;
    if (!args || typeof args !== 'object' || Array.isArray(args)) {
      return;
    }
    const argsMap = args as A2AToolTraceArgs;
    const redacted = argsMap.redacted;
    if (typeof redacted === 'boolean' && redacted) {
      return;
    }
    const effect = this.buildEffectFromTool(toolName, argsMap);
    if (!effect) {
      return;
    }
    if (toolName === 'double_click') {
      this.playEffect(effect);
      setTimeout(() => {
        this.playEffect(effect);
      }, 80);
      return;
    }
    this.playEffect(effect);
  }

  private buildEffectFromTool(toolName: string, args: A2AToolTraceArgs): AgentSpaceViewerEffect | null {
    if (toolName === 'click' || toolName === 'double_click') {
      const point = this.readPoint(args.point);
      if (!point) {
        return null;
      }
      return { kind: 'tap', xNorm: point[0], yNorm: point[1] };
    }
    if (toolName === 'long_click') {
      const point = this.readPoint(args.point);
      if (!point) {
        return null;
      }
      const duration = this.readNumber(args.duration_ms, 1500);
      return { kind: 'long_press', xNorm: point[0], yNorm: point[1], durationMs: duration };
    }
    if (toolName === 'swipe') {
      const start = this.readPoint(args.start) ?? this.readPoint(args.start_point);
      const end = this.readPoint(args.end) ?? this.readPoint(args.end_point);
      if (!start || !end) {
        return null;
      }
      const duration = this.readNumber(args.duration_ms, 300);
      return {
        kind: 'swipe',
        startXNorm: start[0],
        startYNorm: start[1],
        endXNorm: end[0],
        endYNorm: end[1],
        durationMs: duration,
      };
    }
    if (toolName === 'type') {
      const point = this.readPoint(args.point);
      if (!point) {
        return null;
      }
      return { kind: 'tap', xNorm: point[0], yNorm: point[1] };
    }
    return null;
  }

  private readToolTraceRecord(update: A2AStreamUpdate): A2AToolTraceRecord | null {
    const artifactUpdate = update.artifactUpdate;
    const artifact = artifactUpdate?.artifact;
    const parts = artifact?.parts;
    if (!Array.isArray(parts)) {
      return null;
    }
    for (let i = 0; i < parts.length; i += 1) {
      const part = parts[i];
      if (!part || part.kind !== 'data') {
        continue;
      }
      const data = part.data as A2AToolTraceRecord;
      if (data && typeof data === 'object') {
        return data;
      }
    }
    return null;
  }

  private readPoint(raw?: Array<number>): Array<number> | null {
    if (!Array.isArray(raw) || raw.length < 2) {
      return null;
    }
    const x = raw[0];
    const y = raw[1];
    if (typeof x !== 'number' || typeof y !== 'number') {
      return null;
    }
    if (!Number.isFinite(x) || !Number.isFinite(y)) {
      return null;
    }
    return [x, y];
  }

  private readNumber(value: number | undefined, fallback: number): number {
    if (typeof value === 'number' && Number.isFinite(value)) {
      return value;
    }
    return fallback;
  }

  private async flushViewerEffects(): Promise<void> {
    if (this.effectPolling) {
      return;
    }
    this.effectPolling = true;
    try {
      await this.ensureServiceConnected();
      const payload = await this.getServiceClient().pullViewerEffects(this.effectSequence);
      if (payload.length > 0) {
        console.info(`${EFFECT_LOG_TAG} pull effects seq=${this.effectSequence} payloadLen=${payload.length}`);
      }
      const batch = this.parseEffectBatch(payload);
      if (!batch) {
        return;
      }
      console.info(`${EFFECT_LOG_TAG} apply effects seq=${batch.sequence} count=${batch.effects.length} manual=${this.manualControlActive}`);
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
      console.info(`${EFFECT_LOG_TAG} play tap x=${point.x} y=${point.y}`);
      this.interactionController.tap(point);
      return;
    }
    if (effect.kind === 'long_press') {
      const point: InteractionPoint = { x: effect.xNorm, y: effect.yNorm };
      const options = this.resolveEffectOptions(effect.durationMs);
      console.info(`${EFFECT_LOG_TAG} play long_press x=${point.x} y=${point.y} duration=${effect.durationMs ?? 0}`);
      this.interactionController.longPress(point, options);
      return;
    }
    if (effect.kind === 'swipe') {
      const start: InteractionPoint = { x: effect.startXNorm, y: effect.startYNorm };
      const end: InteractionPoint = { x: effect.endXNorm, y: effect.endYNorm };
      const options = this.resolveEffectOptions(effect.durationMs);
      console.info(`${EFFECT_LOG_TAG} play swipe sx=${start.x} sy=${start.y} ex=${end.x} ey=${end.y} duration=${effect.durationMs ?? 0}`);
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

  private updateDisplayVisibility(snapshot: TaskSnapshot): void {
    const appLaunched = snapshot.appLaunched === true;
    if (appLaunched !== this.showDisplay) {
      this.showDisplay = appLaunched;
      if (!appLaunched) {
        this.displaySurfaceInfo = null;
      }
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
      if (name !== 'call_user' && name !== 'interact' && name !== 'take_over') {
        return null;
      }
      const content = action?.parameters?.content ?? '';
      let normalizedContent = typeof content === 'string' ? content.trim() : '';
      if (normalizedContent.length === 0) {
        normalizedContent = snapshot.lastDescription.trim();
      }
      if (normalizedContent.length === 0) {
        return null;
      }
      const options = name === 'interact' ? this.normalizeOptions(action?.parameters?.options) : [];
      const payload: InteractionPayload = {
        kind: name === 'interact' ? 'interact' : name === 'take_over' ? 'take_over' : 'call_user',
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

  private isTakeOverKind(kind: 'call_user' | 'interact' | 'take_over'): boolean {
    return kind === 'take_over';
  }

  private isUserReplyStatusLocked(snapshot: TaskSnapshot): boolean {
    if (!this.lastStatusText.startsWith('已收到你的输入信息') &&
      !this.lastStatusText.startsWith('任务恢复执行')) {
      return false;
    }
    return snapshot.lastActionName === 'call_user' ||
      snapshot.lastActionName === 'interact' ||
      snapshot.lastActionName === 'take_over';
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
      void this.handleCallUserSend(text);
    });
    this.statusController.setCallUserOptionHandler((label: string) => {
      this.onInteractionOption(label);
    });
  }

  private enterInteraction(payload: InteractionPayload): void {
    this.callUserActive = true;
    this.callUserKind = payload.kind;
    this.callUserContent = payload.content;
    this.supplementActive = false;
    this.statusController.setStopped(false);
    this.statusController.setCallUserContent(payload.content);
    this.statusController.clearCallUserInput();
    if (payload.kind === 'interact') {
      this.statusController.setCallUserOptions(payload.options);
    } else {
      this.statusController.setCallUserOptions([]);
    }
    if (this.isTakeOverKind(payload.kind)) {
      this.statusController.setCallUserActionLabel('手动操作');
      this.statusController.setMode('take_over_wait');
    } else {
      this.statusController.setCallUserActionLabel('告诉AIOS');
      this.statusController.setMode('call_user_wait');
    }
    this.showFloatWindow();
  }

  private exitCallUser(): void {
    this.callUserActive = false;
    this.callUserContent = '';
    this.callUserKind = 'call_user';
    this.supplementActive = false;
    this.statusController.clearCallUserInput();
    this.statusController.setCallUserOptions([]);
    this.statusController.setMode('default');
    this.stopManualControl();
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

  private async onTakeoverRequested(): Promise<void> {
    if (this.manualControlActive) {
      return;
    }
    if (this.callUserActive) {
      if (this.isTakeOverKind(this.callUserKind)) {
        this.startManualControl();
      }
      return;
    }
    const content = this.manualTakeOverRequestContent;
    try {
      await this.ensureServiceConnected();
      await this.getServiceClient().requestTakeOver(content);
    } catch (e) {
      console.error(`[AgentSpaceViewer] request take_over failed: ${String(e)}`);
      return;
    }
    this.callUserActive = true;
    this.callUserKind = 'take_over';
    this.callUserContent = content;
    this.statusController.setStopped(false);
    this.statusController.setCallUserContent(content);
    this.statusController.clearCallUserInput();
    this.statusController.setCallUserOptions([]);
    this.statusController.setCallUserActionLabel('手动操作');
    this.statusController.setMode('take_over_wait');
    this.startManualControl();
  }

  private async onSupplementRequested(): Promise<void> {
    if (this.manualControlActive) {
      return;
    }
    if (this.callUserActive) {
      if (this.supplementActive) {
        this.enterCallUserInput();
      }
      return;
    }
    const content = this.supplementRequestContent;
    try {
      await this.ensureServiceConnected();
      await this.getServiceClient().requestSupplement(content);
    } catch (e) {
      console.error(`[AgentSpaceViewer] request supplement failed: ${String(e)}`);
      return;
    }
    this.supplementActive = true;
    this.callUserActive = true;
    this.callUserKind = 'call_user';
    this.callUserContent = content;
    this.statusController.setStopped(false);
    this.statusController.setCallUserContent(content);
    this.statusController.clearCallUserInput();
    this.statusController.setCallUserOptions([]);
    this.statusController.setCallUserActionLabel('补充');
    this.statusController.setMode('call_user_input');
    this.showFloatWindow();
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
    this.stopEffectPolling();
    this.stopStatusPolling();
    try {
      await this.ensureServiceConnected();
      if (this.statusController.stopped) {
        await this.getServiceClient().closeLiveViewNow();
      }
      await this.getServiceClient().transferToMainAndReleaseIfFinished();
    } catch (e) {
      console.error(`[AgentSpaceViewer] transfer to main failed: ${String(e)}`);
    }
    this.terminateViewer();
  }

  private terminateViewer(): void {
    try {
      const ctx = getContext(this) as common.UIAbilityContext;
      ctx.terminateSelf((err: BusinessError) => {
        if (err?.code) {
          console.error(`[AgentSpaceViewer] terminateSelf failed. Code:${err.code}, message:${err.message}`);
          router.back();
        }
      });
    } catch (e) {
      console.error(`[AgentSpaceViewer] terminateSelf exception: ${String(e)}`);
      router.back();
    }
  }

  private enterCallUserWait(): void {
    if (!this.callUserActive) {
      return;
    }
    if (this.isTakeOverKind(this.callUserKind)) {
      this.statusController.setMode('take_over_wait');
    } else {
      this.statusController.setMode('call_user_wait');
    }
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

  private async handleCallUserSend(text: string): Promise<void> {
    if (this.supplementActive) {
      await this.submitSupplementNote(text);
      return;
    }
    await this.submitCallUserReply(text);
  }

  private async submitSupplementNote(text: string): Promise<void> {
    const trimmed = text.trim();
    if (trimmed.length === 0) {
      return;
    }
    try {
      await this.ensureServiceConnected();
      await this.getServiceClient().submitUserNote(trimmed);
      this.callUserActive = false;
      this.callUserContent = '';
      this.callUserKind = 'call_user';
      this.supplementActive = false;
      this.statusController.clearCallUserInput();
      this.statusController.setCallUserOptions([]);
      this.statusController.setMode('default');
      const message = `已收到你的输入信息：\n「${trimmed}」`;
      this.lastStatusText = message;
      this.statusController.setStatus(message);
      this.showFloatWindow();
    } catch (e) {
      console.error(`[AgentSpaceViewer] submit supplement failed: ${String(e)}`);
    }
  }

  private onInteractionOption(label: string): void {
    if (this.callUserKind === 'call_user') {
      this.enterCallUserInput();
      return;
    }
    if (this.isTakeOverKind(this.callUserKind)) {
      this.startManualControl();
      return;
    }
    void this.submitUserReply('interact', label);
  }

  private async submitUserReply(
    kind: 'call_user' | 'interact' | 'take_over',
    text: string,
    displayOverride?: string
  ): Promise<void> {
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
      this.supplementActive = false;
      this.statusController.clearCallUserInput();
      this.statusController.setCallUserOptions([]);
      this.statusController.setMode('default');
      const message = displayOverride ?? `已收到你的输入信息：\n「${trimmed}」`;
      this.lastStatusText = message;
      this.statusController.setStatus(message);
      this.showFloatWindow();
    } catch (e) {
      console.error(`[AgentSpaceViewer] submit user reply failed: ${String(e)}`);
    }
  }

  private startManualControl(): void {
    if (!this.callUserActive || !this.isTakeOverKind(this.callUserKind)) {
      return;
    }
    this.manualControlActive = true;
    this.takeOverPreviousMode = this.viewerMode;
    void this.updateViewerMode('control');
    this.showStatus = false;
    this.showOrb = true;
    this.showGlow = true;
    this.orbController.setMode('compact');
    this.orbController.setText(this.orbDefaultText);
    this.orbController.show();
    this.resetTakeOverIdleTimer();
  }

  private stopManualControl(): void {
    this.clearTakeOverIdleTimer();
    this.stopTakeOverCountdown();
    if (!this.manualControlActive) {
      return;
    }
    this.manualControlActive = false;
    this.orbController.setMode('compact');
    this.orbController.setText(this.orbDefaultText);
    void this.updateViewerMode(this.takeOverPreviousMode);
  }

  private recordManualControlActivity(): void {
    if (!this.manualControlActive) {
      return;
    }
    this.resetTakeOverIdleTimer();
    if (this.takeOverCountdownTimerId !== -1) {
      this.stopTakeOverCountdown();
      this.orbController.setMode('compact');
      this.orbController.setText(this.orbDefaultText);
    }
  }

  private resetTakeOverIdleTimer(): void {
    this.clearTakeOverIdleTimer();
    if (!this.manualControlActive) {
      return;
    }
    this.takeOverIdleTimerId = setTimeout(() => {
      this.takeOverIdleTimerId = -1;
      this.handleTakeOverIdle();
    }, this.takeOverIdleMs);
  }

  private clearTakeOverIdleTimer(): void {
    if (this.takeOverIdleTimerId === -1) {
      return;
    }
    clearTimeout(this.takeOverIdleTimerId);
    this.takeOverIdleTimerId = -1;
  }

  private handleTakeOverIdle(): void {
    if (!this.manualControlActive) {
      return;
    }
    this.startTakeOverCountdown();
  }

  private startTakeOverCountdown(): void {
    if (!this.manualControlActive) {
      return;
    }
    this.stopTakeOverCountdown();
    this.takeOverCountdownValue = this.takeOverCountdownStart;
    this.orbController.setMode('expanded');
    this.orbController.setText(this.formatTakeOverCountdownText(this.takeOverCountdownValue));
    this.takeOverCountdownTimerId = setInterval(() => {
      if (!this.manualControlActive) {
        this.stopTakeOverCountdown();
        return;
      }
      this.takeOverCountdownValue -= 1;
      if (this.takeOverCountdownValue <= 0) {
        this.stopTakeOverCountdown();
        void this.completeTakeOver();
        return;
      }
      this.orbController.setText(this.formatTakeOverCountdownText(this.takeOverCountdownValue));
    }, 1000);
  }

  private stopTakeOverCountdown(): void {
    if (this.takeOverCountdownTimerId === -1) {
      return;
    }
    clearInterval(this.takeOverCountdownTimerId);
    this.takeOverCountdownTimerId = -1;
  }

  private formatTakeOverCountdownText(value: number): string {
    const safeValue = Math.max(0, Math.floor(value));
    return `任务即将恢复(${safeValue})`;
  }

  private async completeTakeOver(): Promise<void> {
    if (!this.manualControlActive) {
      return;
    }
    this.stopManualControl();
    const reply = this.manualTakeOverReply;
    const message = `任务恢复执行\n${reply}`;
    await this.submitUserReply('take_over', reply, message);
  }

  private async updateViewerMode(mode: AgentSpaceDisplayMode): Promise<void> {
    if (this.viewerMode === mode) {
      return;
    }
    this.viewerMode = mode;
    try {
      await this.ensureServiceConnected();
      await this.getServiceClient().setViewerMode(mode);
    } catch (e) {
      console.error(`[AgentSpaceViewer] set viewer mode failed: ${String(e)}`);
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

  private onOrbClick(): void {
    if (this.manualControlActive) {
      return;
    }
    this.showFloatWindow();
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
