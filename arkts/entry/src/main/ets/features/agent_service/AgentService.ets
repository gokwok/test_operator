import type { common, Want } from '@kit.AbilityKit';
import image from '@ohos.multimedia.image';

import { AgentSpace } from '../agentspace/AgentSpace';
import type {
  AgentAction,
  ActionParameters,
  ActionResult,
  AgentSpacePointerEvent,
  AgentSpaceViewerEffectBatch,
  AgentSpaceViewerMode
} from '../agentspace/AgentSpace';
import type { AppTarget, StartAbilityInvoker } from '../agentspace/VTSBackend';
import { CloudAgentClient } from './CloudAgentClient';
import { DeviceExecutor } from './DeviceExecutor';
import type {
  AppTargetDescriptor,
  TaskEventBatch,
  TaskSnapshot,
  TaskStartRequest,
  TaskStatus,
  UserReply
} from './types';
import { TaskManager, TaskManagerListener } from './TaskManager';
import { LiveViewController, LiveViewContents, LiveViewType } from '../live_view/LiveViewController';
import type { AgentSettingsData } from '../settings/AgentSettings';
import { AgentSettings } from '../settings/AgentSettings';
import { AppState } from '../state/app_state';

const DEFAULT_CLOUD_ENDPOINT: string = 'http://114.55.173.20:8005';
const DEFAULT_TASK_PLACEHOLDER: string = '正在分析用户需求...';
const CAPSULE_RUNNING: string = '操作应用';
const CAPSULE_INTERACTION: string = '需要交互';
const CAPSULE_FINISHED: string = '任务完成';
const CAPSULE_STOPPED: string = '任务已停止';
const CAPSULE_FAILED: string = '任务异常';
const FLOAT_WINDOW_ABILITY_NAME: string = 'FloatWindowAbility';
const SELF_BUNDLE_NAME: string = 'com.huawei.aios.agent.operation';
const LIVE_VIEW_AUTO_STOP_DELAY_MS: number = 2000;

export class AgentService {
  private static instance: AgentService | null = null;
  private ctx: common.Context | null = null;
  private startAbilityInvoker: StartAbilityInvoker | null = null;
  private readonly createdAtMs: number = Date.now();
  private taskManager: TaskManager = new TaskManager();
  private deviceExecutor: DeviceExecutor | null = null;
  private cloudClient: CloudAgentClient | null = null;
  private lastScreenshotBase64: string = '';
  private liveViewController: LiveViewController = LiveViewController.getInstance();
  private liveViewEnabled: boolean = true;
  private liveViewStarted: boolean = false;
  private liveViewPhase: TaskStatus = 'idle';
  private liveViewTaskText: string = '';
  private liveViewIcon: image.PixelMap | null = null;
  private liveViewAppName: string = '';
  private liveViewNotificationId: number = 0;
  private liveViewSeq: number = 0;
  private listenerBound: boolean = false;

  static getInstance(): AgentService {
    if (!AgentService.instance) {
      AgentService.instance = new AgentService();
    }
    return AgentService.instance;
  }

  init(ctx: common.Context, startAbilityInvoker: StartAbilityInvoker): void {
    this.ctx = ctx;
    this.startAbilityInvoker = startAbilityInvoker;
  }

  applySettings(settings: AgentSettingsData): void {
    this.liveViewEnabled = settings.liveViewEnabled;
  }

  ping(): string {
    return `agent_service:${this.createdAtMs}`;
  }

  getSnapshotJson(): string {
    return JSON.stringify(this.taskManager.getSnapshot());
  }

  getContext(): common.Context | null {
    return this.ctx;
  }

  startTask(request: TaskStartRequest): void {
    this.ensureListenerBound();
    const normalized = this.normalizeStartRequest(request);
    const executor = this.getDeviceExecutor();
    const targets = Array.isArray(normalized.appTargets) ? normalized.appTargets : [];
    executor.setAppTargets(targets);
    const cloud = this.getCloudClient();
    this.liveViewTaskText = normalized.taskText;
    this.liveViewPhase = 'running';
    this.liveViewStarted = false;
    this.liveViewAppName = '';
    this.liveViewIcon = null;
    this.liveViewNotificationId = this.allocateLiveViewId();
    this.taskManager.startTask(normalized, cloud, executor);
    void this.publishRunningLiveView();
  }

  async startTaskWithDefaults(taskText: string): Promise<void> {
    const trimmed = taskText.trim();
    if (trimmed.length === 0) {
      throw new Error('task_text_empty');
    }
    const ctx = this.ctx;
    if (!ctx) {
      throw new Error('context_missing');
    }
    const settings = await AgentSettings.load(ctx);
    const installedApps = await this.safeListAppNames(ctx);
    const appTargets = await this.safeListAppTargets(ctx);
    const request: TaskStartRequest = {
      sessionId: '',
      taskText: trimmed,
      installedApps,
      appTargets,
      waitMs: settings.waitMs,
    };
    this.applySettings(settings);
    if (settings.cloudEndpoint.length > 0) {
      this.setCloudEndpoint(settings.cloudEndpoint);
    }
    this.startTask(request);
    await this.hideFloatWindow();
  }

  private async safeListAppNames(ctx: common.Context): Promise<string[]> {
    try {
      return await AppState.listAppNames(ctx);
    } catch (e) {
      console.warn(`[AgentService] listAppNames failed: ${String(e)}`);
      return [];
    }
  }

  private async safeListAppTargets(ctx: common.Context): Promise<AppTargetDescriptor[]> {
    try {
      return await AppState.listAppTargets(ctx);
    } catch (e) {
      console.warn(`[AgentService] listAppTargets failed: ${String(e)}`);
      return [];
    }
  }

  stopTask(reason: string): void {
    this.taskManager.stopTask(reason);
  }

  async hideFloatWindow(): Promise<void> {
    const ctx = this.ctx;
    if (!ctx) {
      return;
    }
    const want: Want = {
      bundleName: SELF_BUNDLE_NAME,
      abilityName: FLOAT_WINDOW_ABILITY_NAME,
      action: 'hide',
    };
    const extCtx = ctx as common.ServiceExtensionContext;
    try {
      await extCtx.startServiceExtensionAbility(want);
      return;
    } catch (_e) {
      // fallback to startAbility when service extension start is unavailable
    }
    await extCtx.startAbility(want);
  }

  pullEventsJson(lastSequence: number): string {
    const batch: TaskEventBatch | null = this.taskManager.pullEvents(lastSequence);
    if (!batch) {
      return '';
    }
    return JSON.stringify(batch);
  }

  submitUserReply(reply: UserReply): void {
    this.taskManager.submitUserReply(reply);
  }

  debugCallUser(content: string): void {
    this.taskManager.debugCallUser(content);
  }

  debugTakeOver(content: string): void {
    this.taskManager.debugTakeOver(content);
  }

  debugInteract(content: string, options: Array<string>): void {
    this.taskManager.debugInteract(content, options);
  }

  setCloudEndpoint(endpoint: string): void {
    this.getCloudClient().setEndpoint(endpoint);
  }

  async ensureVtsJson(): Promise<string> {
    const runtime = await this.getDeviceExecutor().ensureVts();
    return JSON.stringify(runtime);
  }

  async getRuntimeJson(): Promise<string> {
    const runtime = await this.getDeviceExecutor().getRuntimeInfo();
    return JSON.stringify(runtime);
  }

  async launchApp(bundleName: string, abilityName: string): Promise<void> {
    const target: AppTarget = { bundleName, abilityName };
    await this.getDeviceExecutor().launchApp(target);
  }

  async clickNorm(xNorm: number, yNorm: number): Promise<void> {
    await this.getDeviceExecutor().clickNorm(xNorm, yNorm);
  }

  async captureScreenshotLength(): Promise<string> {
    const base64 = await this.getDeviceExecutor().screenshotBase64();
    this.lastScreenshotBase64 = base64;
    return String(base64.length);
  }

  readScreenshotChunk(offset: number, size: number): string {
    if (this.lastScreenshotBase64.length === 0) {
      return '';
    }
    const safeOffset = Math.max(0, Math.floor(offset));
    const safeSize = Math.max(0, Math.floor(size));
    if (safeSize === 0 || safeOffset >= this.lastScreenshotBase64.length) {
      return '';
    }
    const end = Math.min(this.lastScreenshotBase64.length, safeOffset + safeSize);
    return this.lastScreenshotBase64.substring(safeOffset, end);
  }

  async destroyVts(): Promise<void> {
    await this.getDeviceExecutor().destroy();
  }

  async stopVts(): Promise<number> {
    return await this.getDeviceExecutor().stop();
  }

  async transferToMain(): Promise<number> {
    return await this.getDeviceExecutor().transferToMain();
  }

  async actionDoubleClick(xNorm: number, yNorm: number): Promise<string> {
    const params: ActionParameters = { point: [xNorm, yNorm] };
    const action: AgentAction = { name: 'double_click', parameters: params };
    return await this.executeAction(action);
  }

  async actionLongClick(xNorm: number, yNorm: number, durationMs: number): Promise<string> {
    const params: ActionParameters = { point: [xNorm, yNorm], duration_ms: durationMs };
    const action: AgentAction = { name: 'long_click', parameters: params };
    return await this.executeAction(action);
  }

  async actionSwipe(
    startXNorm: number,
    startYNorm: number,
    endXNorm: number,
    endYNorm: number,
    durationMs: number
  ): Promise<string> {
    const params: ActionParameters = {
      start_point: [startXNorm, startYNorm],
      end_point: [endXNorm, endYNorm],
      duration_ms: durationMs,
    };
    const action: AgentAction = { name: 'swipe', parameters: params };
    return await this.executeAction(action);
  }

  async actionType(content: string, hasPoint: boolean, xNorm: number, yNorm: number): Promise<string> {
    const params: ActionParameters = { content };
    if (hasPoint) {
      params.point = [xNorm, yNorm];
    }
    const action: AgentAction = { name: 'type', parameters: params };
    return await this.executeAction(action);
  }

  async actionPressBack(): Promise<string> {
    const action: AgentAction = { name: 'press_back' };
    return await this.executeAction(action);
  }

  async actionPressEnter(): Promise<string> {
    const action: AgentAction = { name: 'press_enter' };
    return await this.executeAction(action);
  }

  async setViewerActive(active: boolean): Promise<void> {
    await this.ensureAgentSpace().setViewerActive(active);
  }

  async getViewerMode(): Promise<AgentSpaceViewerMode> {
    return await this.ensureAgentSpace().getViewerMode();
  }

  async setViewerMode(mode: AgentSpaceViewerMode): Promise<void> {
    await this.ensureAgentSpace().setViewerMode(mode);
  }

  async attachViewerSurface(surfaceId: string): Promise<void> {
    await this.ensureAgentSpace().attachViewerSurface(surfaceId);
  }

  private ensureListenerBound(): void {
    if (this.listenerBound) {
      return;
    }
    const listener: TaskManagerListener = new AgentServiceTaskListener(this);
    this.taskManager.setListener(listener);
    this.listenerBound = true;
  }

  async handleTaskStatusChanged(snapshot: TaskSnapshot): Promise<void> {
    if (!this.liveViewEnabled) {
      return;
    }
    const status = snapshot.status;
    if (status === this.liveViewPhase && status !== 'waiting_user') {
      return;
    }
    this.liveViewPhase = status;
    if (status === 'running') {
      await this.publishRunningLiveView();
      return;
    }
    if (status === 'waiting_user') {
      const content = this.resolveInteractionContent(snapshot);
      await this.publishInteractionLiveView(content);
      return;
    }
    if (status === 'finished') {
      await this.publishFinishedLiveView();
      return;
    }
    if (status === 'failed') {
      await this.publishFailedLiveView(snapshot.lastError);
      return;
    }
    if (status === 'stopping') {
      await this.publishStoppedLiveView(snapshot.lastError);
    }
  }

  async handleTaskAction(action: AgentAction, description: string): Promise<void> {
    if (!this.liveViewEnabled) {
      return;
    }
    const name = action.name.trim().toLowerCase();
    if (name === 'launch_app') {
      const appName = this.resolveAppName(action);
      if (appName.length > 0) {
        await this.updateLiveViewIcon(appName);
      }
      return;
    }
    if (name === 'call_user' || name === 'interact' || name === 'take_over' || name === 'task_over') {
      const content = this.resolveActionContent(action, description);
      await this.publishInteractionLiveView(content);
    }
  }

  private resolveActionContent(action: AgentAction, fallback: string): string {
    const content = action.parameters?.content ?? '';
    if (typeof content === 'string' && content.trim().length > 0) {
      return content.trim();
    }
    const trimmed = fallback.trim();
    return trimmed.length > 0 ? trimmed : DEFAULT_TASK_PLACEHOLDER;
  }

  private resolveInteractionContent(snapshot: TaskSnapshot): string {
    const json = snapshot.lastActionJson.trim();
    if (json.length > 0) {
      try {
        const parsed = JSON.parse(json) as AgentAction;
        return this.resolveActionContent(parsed, snapshot.lastDescription);
      } catch (_e) {
        // ignore
      }
    }
    return snapshot.lastDescription.trim().length > 0 ? snapshot.lastDescription.trim() : DEFAULT_TASK_PLACEHOLDER;
  }

  private resolveAppName(action: AgentAction): string {
    const name = action.parameters?.app ?? '';
    if (typeof name !== 'string') {
      return '';
    }
    return name.trim();
  }

  private async publishRunningLiveView(): Promise<void> {
    if (!this.liveViewEnabled) {
      return;
    }
    const ctx = this.ctx;
    if (!ctx) {
      return;
    }
    const icon = await this.resolveLiveViewIcon();
    const content = this.liveViewTaskText.trim().length > 0 ? this.liveViewTaskText.trim() : DEFAULT_TASK_PLACEHOLDER;
    const payload = this.buildLiveViewPayload('任务执行中', content, CAPSULE_RUNNING, icon);
    await this.publishLiveView(payload);
  }

  private async publishInteractionLiveView(content: string): Promise<void> {
    if (!this.liveViewEnabled) {
      return;
    }
    const ctx = this.ctx;
    if (!ctx) {
      return;
    }
    const icon = await this.resolveLiveViewIcon();
    const payload = this.buildLiveViewPayload('等待答复', content, CAPSULE_INTERACTION, icon);
    await this.publishLiveView(payload);
  }

  private async publishFinishedLiveView(): Promise<void> {
    if (!this.liveViewEnabled) {
      return;
    }
    const ctx = this.ctx;
    if (!ctx) {
      return;
    }
    const icon = await this.resolveLiveViewIcon();
    const payload = this.buildLiveViewPayload('任务完成', '', CAPSULE_FINISHED, icon);
    await this.stopLiveViewWithPayload(payload, LIVE_VIEW_AUTO_STOP_DELAY_MS);
  }

  private async publishStoppedLiveView(reason: string): Promise<void> {
    if (!this.liveViewEnabled) {
      return;
    }
    const ctx = this.ctx;
    if (!ctx) {
      return;
    }
    const icon = await this.resolveLiveViewIcon();
    const content = reason.trim().length > 0 ? reason.trim() : '';
    const payload = this.buildLiveViewPayload('任务已停止', content, CAPSULE_STOPPED, icon);
    await this.stopLiveViewWithPayload(payload, LIVE_VIEW_AUTO_STOP_DELAY_MS);
  }

  private async publishFailedLiveView(reason: string): Promise<void> {
    if (!this.liveViewEnabled) {
      return;
    }
    const ctx = this.ctx;
    if (!ctx) {
      return;
    }
    const icon = await this.resolveLiveViewIcon();
    const content = reason.trim().length > 0 ? reason.trim() : '';
    const payload = this.buildLiveViewPayload('任务异常', content, CAPSULE_FAILED, icon);
    await this.stopLiveViewWithPayload(payload, LIVE_VIEW_AUTO_STOP_DELAY_MS);
  }

  private buildLiveViewPayload(
    title: string,
    content: string,
    capsuleTitle: string,
    icon: image.PixelMap | null
  ): LiveViewContents {
    const payload: LiveViewContents = {
      type: LiveViewType.VIEWER,
      id: this.liveViewNotificationId > 0 ? this.liveViewNotificationId : undefined,
      primary: { title, content },
      capsule: { title: capsuleTitle },
    };
    if (icon) {
      payload.capsuleIcon = icon;
    }
    return payload;
  }

  private async publishLiveView(payload: LiveViewContents): Promise<void> {
    if (!this.liveViewEnabled) {
      return;
    }
    const ctx = this.ctx;
    if (!ctx) {
      return;
    }
    try {
      if (!this.liveViewStarted) {
        await this.liveViewController.start(ctx, payload);
        this.liveViewStarted = true;
      } else {
        await this.liveViewController.update(ctx, payload);
      }
    } catch (e) {
      console.warn(`[AgentService] LiveView update failed: ${String(e)}`);
    }
  }

  private async stopLiveViewWithPayload(payload: LiveViewContents, delayMs: number): Promise<void> {
    if (!this.liveViewEnabled) {
      return;
    }
    const ctx = this.ctx;
    if (!ctx) {
      return;
    }
    payload.stopTimeoutMs = Math.max(0, Math.floor(delayMs));
    try {
      await this.liveViewController.stop(ctx, payload);
    } catch (e) {
      console.warn(`[AgentService] LiveView stop failed: ${String(e)}`);
    } finally {
      this.liveViewStarted = false;
    }
  }

  private allocateLiveViewId(): number {
    this.liveViewSeq = (this.liveViewSeq + 1) % 1000;
    const base = Date.now() % 1000000000;
    return Math.floor(base + this.liveViewSeq);
  }

  private async resolveLiveViewIcon(): Promise<image.PixelMap | null> {
    if (this.liveViewIcon) {
      return this.liveViewIcon;
    }
    const ctx = this.ctx;
    if (!ctx) {
      return null;
    }
    try {
      const icon = await AppState.getIconByAppName(SELF_BUNDLE_NAME, ctx);
      this.liveViewIcon = icon;
      return icon;
    } catch (e) {
      console.warn(`[AgentService] load default icon failed: ${String(e)}`);
      return null;
    }
  }

  private async updateLiveViewIcon(appName: string): Promise<void> {
    const ctx = this.ctx;
    if (!ctx) {
      return;
    }
    if (appName === this.liveViewAppName && this.liveViewIcon) {
      return;
    }
    try {
      const icon = await AppState.getIconByAppName(appName, ctx);
      this.liveViewAppName = appName;
      this.liveViewIcon = icon;
    } catch (e) {
      console.warn(`[AgentService] load app icon failed: ${String(e)}`);
      return;
    }
    if (this.liveViewPhase === 'running') {
      await this.publishRunningLiveView();
    }
  }

  private normalizeStartRequest(request: TaskStartRequest): TaskStartRequest {
    const sessionId = request.sessionId.trim().length > 0 ? request.sessionId.trim() : this.generateSessionId();
    const normalized: TaskStartRequest = {
      sessionId,
      taskText: request.taskText,
      installedApps: request.installedApps,
      appTargets: request.appTargets,
      waitMs: request.waitMs,
    };
    return normalized;
  }

  private generateSessionId(): string {
    const rand = Math.floor(Math.random() * 1000);
    return `local_${Date.now()}_${rand}`;
  }

  async sendViewerEvent(event: AgentSpacePointerEvent): Promise<void> {
    await this.ensureAgentSpace().sendViewerEvent(event);
  }

  async pullViewerEffectsJson(lastSequence: number): Promise<string> {
    const batch: AgentSpaceViewerEffectBatch | null = await this.ensureAgentSpace().pullViewerEffects(lastSequence);
    if (!batch) {
      return '';
    }
    return JSON.stringify(batch);
  }

  private async executeAction(action: AgentAction): Promise<string> {
    const result = await this.getDeviceExecutor().executeAction(action);
    return this.formatActionResult(result);
  }

  private formatActionResult(result: ActionResult): string {
    const status = result.status;
    const error = result.error ?? '';
    if (error.length > 0) {
      return `${status}:${error}`;
    }
    return status;
  }

  private ensureAgentSpace(): AgentSpace {
    const invoker = this.startAbilityInvoker;
    if (!invoker) {
      throw new Error('Service startAbility not initialized');
    }
    return AgentSpace.init(invoker);
  }

  private getDeviceExecutor(): DeviceExecutor {
    if (!this.deviceExecutor) {
      this.deviceExecutor = new DeviceExecutor(this.ensureAgentSpace());
    }
    return this.deviceExecutor;
  }

  private getCloudClient(): CloudAgentClient {
    if (!this.cloudClient) {
      this.cloudClient = new CloudAgentClient(DEFAULT_CLOUD_ENDPOINT);
    }
    return this.cloudClient;
  }
}

class AgentServiceTaskListener implements TaskManagerListener {
  private readonly owner: AgentService;

  constructor(owner: AgentService) {
    this.owner = owner;
  }

  onStatusChanged(snapshot: TaskSnapshot): void {
    void this.owner.handleTaskStatusChanged(snapshot);
  }

  onAction(action: AgentAction, description: string): void {
    void this.owner.handleTaskAction(action, description);
  }
}
