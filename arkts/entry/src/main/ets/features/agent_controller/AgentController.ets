import type { common, Want } from '@kit.AbilityKit';
import type { BusinessError } from '@kit.BasicServicesKit';
import image from '@ohos.multimedia.image';
import bundleManager from '@ohos.bundle.bundleManager';
import bundleResourceManager from '@ohos.bundle.bundleResourceManager';
import backgroundTaskManager from '@ohos.resourceschedule.backgroundTaskManager';
import { hilog } from '@kit.PerformanceAnalysisKit';

import { AgentSpace } from './agentspace/AgentSpace';
import type {
  AgentAction,
  ActionParameters,
  ActionResult,
  AgentSpacePointerEvent,
  AgentSpaceViewerEffectBatch,
  AgentSpaceViewerMode
} from './agentspace/AgentSpace';
import type { AppTarget, StartAbilityInvoker } from './agentspace/VTSBackend';
import { AgentProxy } from './AgentProxy';
import { StateCollector } from './state_collector/StateCollector';
import { ToolKit } from './toolkit/ToolKit';
import type {
  AppTargetDescriptor,
  TaskEventBatch,
  TaskSnapshot,
  TaskStartRequest,
  TaskStatus,
  UserReply
} from './types';
import { TaskManager, TaskManagerListener } from './task_manager/TaskManager';
import { LiveViewController, LiveViewContents, LiveViewType } from './live_view/LiveViewController';
import { LiveViewPresenter, LiveViewPresenterContext } from './live_view/LiveViewPresenter';
import type { AgentSettingsData } from '../persistence/settings/AgentSettings';
import { AgentSettings } from '../persistence/settings/AgentSettings';
import { AppState } from './state_collector/AppState';
import { TaskHistoryStore } from '../persistence/history/TaskHistoryStore';

const DEFAULT_CLOUD_ENDPOINT: string = 'http://114.55.173.20:8005';
const FLOAT_WINDOW_ABILITY_NAME: string = 'FloatWindowAbility';
const SELF_BUNDLE_NAME: string = 'com.huawei.aios.agent.operation';
const LIVE_VIEW_AUTO_STOP_DELAY_MS: number = 30000;
const EFFICIENCY_RESOURCE_TIMEOUT_MS: number = 60 * 60 * 1000;
const EFFICIENCY_RESOURCE_REASON: string = 'agent_task';
const HILOG_DOMAIN: number = 0x0000;
const HILOG_TAG: string = 'AgentController';

export class AgentController {
  private static instance: AgentController | null = null;
  private ctx: common.Context | null = null;
  private startAbilityInvoker: StartAbilityInvoker | null = null;
  private readonly createdAtMs: number = Date.now();
  private taskManager: TaskManager = new TaskManager();
  private agentSpace: AgentSpace | null = null;
  private toolKit: ToolKit | null = null;
  private agentProxy: AgentProxy | null = null;
  private appTargetMap: Map<string, AppTarget> = new Map<string, AppTarget>();
  private lastLaunchTarget: AppTarget | null = null;
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
  private efficiencyApplied: boolean = false;
  private liveViewCancelBound: boolean = false;
  private liveViewSelfStopping: boolean = false;
  private viewerActive: boolean = false;
  private pendingAgentSpaceDestroy: boolean = false;
  private historyStore: TaskHistoryStore = TaskHistoryStore.getInstance();
  private agentSpaceOp: Promise<void> = Promise.resolve();

  static getInstance(): AgentController {
    if (!AgentController.instance) {
      AgentController.instance = new AgentController();
    }
    return AgentController.instance;
  }

  init(ctx: common.Context, startAbilityInvoker: StartAbilityInvoker): void {
    this.ctx = ctx;
    this.startAbilityInvoker = startAbilityInvoker;
    this.historyStore.init(ctx);
  }

  applySettings(settings: AgentSettingsData): void {
    this.liveViewEnabled = settings.liveViewEnabled;
    this.historyStore.configure(settings.historyEnabled, settings.historyMaxTasks);
  }

  ping(): string {
    return `agent_controller:${this.createdAtMs}`;
  }

  getSnapshotJson(): string {
    const snapshot = this.taskManager.getSnapshot();
    const appLaunched = this.agentSpace ? this.agentSpace.isAppLaunched() : false;
    snapshot.appLaunched = appLaunched;
    return JSON.stringify(snapshot);
  }

  getContext(): common.Context | null {
    return this.ctx;
  }

  async startTask(request: TaskStartRequest): Promise<void> {
    this.ensureListenerBound();
    this.bindLiveViewCancelListener();
    const normalized = this.normalizeStartRequest(request);
    await this.resetAgentSpaceForNewTask();
    const toolKit = this.requireToolKit();
    const targets = Array.isArray(normalized.appTargets) ? normalized.appTargets : [];
    toolKit.setAppTargets(targets);
    this.appTargetMap = this.buildAppTargetMap(targets);
    this.lastLaunchTarget = null;
    const agentProxy = this.getAgentProxy();
    this.liveViewTaskText = normalized.taskText;
    this.liveViewPhase = 'running';
    this.liveViewStarted = false;
    this.liveViewAppName = '';
    this.liveViewIcon = null;
    this.liveViewNotificationId = this.allocateLiveViewId();
    const collector = new StateCollector(toolKit);
    this.pendingAgentSpaceDestroy = false;
    this.taskManager.startTask(normalized, agentProxy, toolKit, collector);
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
    this.applyEfficiencyResourcesBestEffort();
    await this.startTask(request);
    await this.hideFloatWindow();
  }

  private async safeListAppNames(ctx: common.Context): Promise<string[]> {
    try {
      return await AppState.listAppNames(ctx);
    } catch (e) {
      console.warn(`[AgentController] listAppNames failed: ${String(e)}`);
      return [];
    }
  }

  private async safeListAppTargets(ctx: common.Context): Promise<AppTargetDescriptor[]> {
    try {
      return await AppState.listAppTargets(ctx);
    } catch (e) {
      console.warn(`[AgentController] listAppTargets failed: ${String(e)}`);
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

  requestTaskOver(content: string): void {
    this.taskManager.requestTaskOver(content);
  }

  requestSupplement(content: string): void {
    this.taskManager.requestSupplement(content);
  }

  submitUserNote(content: string): void {
    this.taskManager.submitUserNote(content);
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
    this.getAgentProxy().setEndpoint(endpoint);
  }

  async ensureVtsJson(): Promise<string> {
    const runtime = await this.requireToolKit().ensureVts();
    return JSON.stringify(runtime);
  }

  async getRuntimeJson(): Promise<string> {
    const runtime = await this.requireToolKit().getRuntimeInfo();
    return JSON.stringify(runtime);
  }

  async launchApp(bundleName: string, abilityName: string): Promise<void> {
    const target: AppTarget = { bundleName, abilityName };
    this.lastLaunchTarget = target;
    await this.requireToolKit().launchApp(target);
  }

  async clickNorm(xNorm: number, yNorm: number): Promise<void> {
    await this.requireToolKit().clickNorm(xNorm, yNorm);
  }

  async captureScreenshotLength(): Promise<string> {
    const base64 = await this.requireToolKit().screenshotBase64();
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
    await this.requireToolKit().destroy();
  }

  async stopVts(): Promise<number> {
    return await this.requireToolKit().stop();
  }

  async transferToMain(): Promise<number> {
    const toolKit = this.toolKit;
    if (toolKit) {
      try {
        return await toolKit.transferToMain();
      } catch (e) {
        hilog.warn(HILOG_DOMAIN, HILOG_TAG, 'transferToMain via VTS failed: %{public}s', String(e));
      }
    }
    const target = this.lastLaunchTarget;
    if (!target) {
      return 0;
    }
    try {
      await this.startAppOnMain(target);
      return 1;
    } catch (e) {
      hilog.warn(HILOG_DOMAIN, HILOG_TAG, 'transferToMain fallback start failed: %{public}s', String(e));
      return 0;
    }
  }

  async transferToMainAndReleaseIfFinished(): Promise<number> {
    let count = 0;
    try {
      count = await this.transferToMain();
    } catch (e) {
      hilog.warn(HILOG_DOMAIN, HILOG_TAG, 'transferToMain failed: %{public}s', String(e));
      return 0;
    }
    if (count > 0) {
      await this.releaseAgentSpaceIfFinished();
    }
    return count;
  }

  async releaseAgentSpaceIfFinished(): Promise<boolean> {
    const snapshot = this.taskManager.getSnapshot();
    if (snapshot.status !== 'finished') {
      return false;
    }
    if (!this.agentSpace) {
      return false;
    }
    await this.runAgentSpaceOp(async () => {
      await this.destroyAgentSpaceInternal();
    });
    return true;
  }

  async closeLiveViewNow(): Promise<void> {
    if (!this.liveViewEnabled) {
      return;
    }
    const ctx = this.ctx;
    if (!ctx) {
      return;
    }
    if (this.liveViewNotificationId <= 0) {
      return;
    }
    const payload: LiveViewContents = {
      type: LiveViewType.VIEWER,
      id: this.liveViewNotificationId,
      stopTimeoutMs: 0,
      primary: { title: '', content: '' },
      capsule: { title: '' },
    };
    try {
      this.liveViewSelfStopping = true;
      await this.liveViewController.stop(ctx, payload);
    } catch (e) {
      console.warn(`[AgentController] LiveView close failed: ${String(e)}`);
    } finally {
      this.liveViewSelfStopping = false;
      this.liveViewStarted = false;
    }
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
    this.viewerActive = active;
    const space = this.agentSpace;
    if (!space) {
      if (!active && this.pendingAgentSpaceDestroy) {
        this.pendingAgentSpaceDestroy = false;
      }
      return;
    }
    await space.setViewerActive(active);
    if (!active && this.pendingAgentSpaceDestroy) {
      this.pendingAgentSpaceDestroy = false;
      void this.runAgentSpaceOp(async () => {
        await this.destroyAgentSpaceInternal();
      });
    }
  }

  async getViewerMode(): Promise<AgentSpaceViewerMode> {
    const space = this.agentSpace;
    if (!space) {
      return 'display';
    }
    return await space.getViewerMode();
  }

  async setViewerMode(mode: AgentSpaceViewerMode): Promise<void> {
    const space = this.agentSpace;
    if (!space) {
      return;
    }
    await space.setViewerMode(mode);
  }

  async attachViewerSurface(surfaceId: string): Promise<void> {
    const space = this.agentSpace;
    if (!space) {
      return;
    }
    await space.attachViewerSurface(surfaceId);
  }

  private ensureListenerBound(): void {
    if (this.listenerBound) {
      return;
    }
    const listener: TaskManagerListener = new AgentControllerTaskListener(this);
    this.taskManager.setListener(listener);
    this.listenerBound = true;
  }

  async handleTaskStatusChanged(snapshot: TaskSnapshot): Promise<void> {
    const status = snapshot.status;
    if (status === 'finished' || status === 'failed' || status === 'stopping' || status === 'idle') {
      this.releaseEfficiencyResourcesBestEffort();
    }

    if (status === 'finished') {
      // Keep AgentSpace alive so transferToMain can reuse the existing app instance.
      this.pendingAgentSpaceDestroy = false;
    } else if (status === 'failed' || status === 'idle') {
      if (this.viewerActive) {
        this.pendingAgentSpaceDestroy = true;
      } else {
        this.pendingAgentSpaceDestroy = false;
        void this.runAgentSpaceOp(async () => {
          await this.destroyAgentSpaceInternal();
        });
      }
    }

    if (!this.liveViewEnabled) {
      return;
    }
    if (status === this.liveViewPhase && status !== 'waiting_user') {
      return;
    }
    this.liveViewPhase = status;
    if (status === 'running') {
      await this.publishRunningLiveView();
      return;
    }
    if (status === 'waiting_user') {
      const content = LiveViewPresenter.resolveInteractionContent(snapshot);
      await this.publishInteractionLiveView(content);
      return;
    }
    if (status === 'finished') {
      const content = LiveViewPresenter.resolveFinishContent(snapshot);
      await this.publishFinishedLiveView(content);
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
    this.updateLastLaunchTarget(action);
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
      const content = LiveViewPresenter.resolveActionContent(action, description);
      await this.publishInteractionLiveView(content);
    }
  }

  private resolveAppName(action: AgentAction): string {
    const name = action.parameters?.app ?? '';
    if (typeof name !== 'string') {
      return '';
    }
    return name.trim();
  }

  private updateLastLaunchTarget(action: AgentAction): void {
    const name = action.name.trim().toLowerCase();
    if (name !== 'launch_app') {
      return;
    }
    const appName = this.resolveAppName(action);
    if (appName.length === 0) {
      return;
    }
    const target = this.resolveAppTarget(appName);
    if (target) {
      this.lastLaunchTarget = target;
    }
  }

  private resolveAppTarget(rawName: string): AppTarget | null {
    const name = rawName.trim();
    if (name.length === 0) {
      return null;
    }
    const direct = this.parseDirectTarget(name);
    if (direct) {
      return direct;
    }
    const mapped = this.appTargetMap.get(name);
    if (mapped) {
      return mapped;
    }
    const extracted = this.extractBundleNameSuffix(name);
    if (extracted.length > 0) {
      return { bundleName: extracted, abilityName: 'EntryAbility' };
    }
    return null;
  }

  private parseDirectTarget(name: string): AppTarget | null {
    const slashIndex = name.indexOf('/');
    if (slashIndex > 0 && slashIndex < name.length - 1) {
      const bundleName = name.slice(0, slashIndex).trim();
      const abilityName = name.slice(slashIndex + 1).trim();
      if (bundleName.length === 0 || abilityName.length === 0) {
        return null;
      }
      return { bundleName, abilityName };
    }
    if (this.looksLikeBundleName(name)) {
      return { bundleName: name, abilityName: 'EntryAbility' };
    }
    return null;
  }

  private looksLikeBundleName(name: string): boolean {
    const normalized = name.trim();
    if (normalized.length === 0) {
      return false;
    }
    if (normalized.startsWith('com.') || normalized.startsWith('ohos.') || normalized.startsWith('cn.')) {
      return true;
    }
    return normalized.indexOf('.') >= 0;
  }

  private extractBundleNameSuffix(input: string): string {
    if (!input.endsWith(')')) {
      return '';
    }
    const openIndex = input.lastIndexOf('(');
    if (openIndex < 0) {
      return '';
    }
    return input.slice(openIndex + 1, input.length - 1).trim();
  }

  private buildAppTargetMap(targets: Array<AppTargetDescriptor>): Map<string, AppTarget> {
    const map = new Map<string, AppTarget>();
    for (let i = 0; i < targets.length; i++) {
      const entry = targets[i];
      const displayName = entry.displayName.trim();
      const bundleName = entry.bundleName.trim();
      if (displayName.length === 0 || bundleName.length === 0) {
        continue;
      }
      const abilityName = entry.abilityName.trim().length > 0 ? entry.abilityName.trim() : 'EntryAbility';
      map.set(displayName, { bundleName, abilityName });
    }
    return map;
  }

  private async startAppOnMain(target: AppTarget): Promise<void> {
    const invoker = this.startAbilityInvoker;
    if (!invoker) {
      throw new Error('startAbilityInvoker_missing');
    }
    const want: Want = {
      bundleName: target.bundleName,
      abilityName: target.abilityName,
    };
    await invoker(want);
  }

  private buildLiveViewContext(icon: image.PixelMap | null): LiveViewPresenterContext {
    const context: LiveViewPresenterContext = {
      notificationId: this.liveViewNotificationId,
      taskText: this.liveViewTaskText,
      icon,
    };
    return context;
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
    const presenterContext = this.buildLiveViewContext(icon);
    const payload = LiveViewPresenter.buildRunning(presenterContext);
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
    const presenterContext = this.buildLiveViewContext(icon);
    const payload = LiveViewPresenter.buildInteraction(presenterContext, content);
    await this.publishLiveView(payload);
  }

  private async publishFinishedLiveView(content: string): Promise<void> {
    if (!this.liveViewEnabled) {
      return;
    }
    const ctx = this.ctx;
    if (!ctx) {
      return;
    }
    const icon = await this.resolveLiveViewIcon();
    const presenterContext = this.buildLiveViewContext(icon);
    const payload = LiveViewPresenter.buildFinished(presenterContext, content);
    void this.stopLiveViewWithPayload(payload, LIVE_VIEW_AUTO_STOP_DELAY_MS);
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
    const presenterContext = this.buildLiveViewContext(icon);
    const payload = LiveViewPresenter.buildStopped(presenterContext, reason);
    void this.stopLiveViewWithPayload(payload, LIVE_VIEW_AUTO_STOP_DELAY_MS);
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
    const presenterContext = this.buildLiveViewContext(icon);
    const payload = LiveViewPresenter.buildFailed(presenterContext, reason);
    void this.stopLiveViewWithPayload(payload, LIVE_VIEW_AUTO_STOP_DELAY_MS);
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
      console.warn(`[AgentController] LiveView update failed: ${String(e)}`);
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
      this.liveViewSelfStopping = true;
      await this.liveViewController.stop(ctx, payload);
    } catch (e) {
      console.warn(`[AgentController] LiveView stop failed: ${String(e)}`);
    } finally {
      this.liveViewSelfStopping = false;
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
      if (await this.isPixelMapValid(icon)) {
        this.liveViewIcon = icon;
        return icon;
      }
    } catch (e) {
      console.warn(`[AgentController] load default icon failed: ${String(e)}`);
    }
    const fallback = await this.loadSelfBundleIcon();
    if (fallback) {
      this.liveViewIcon = fallback;
      return fallback;
    }
    return null;
  }

  private async loadSelfBundleIcon(): Promise<image.PixelMap | null> {
    try {
      const appFlags: number = bundleManager.ApplicationFlag.GET_APPLICATION_INFO_DEFAULT |
        bundleManager.ApplicationFlag.GET_APPLICATION_INFO_WITH_DISABLE;
      const applicationInfo = await bundleManager.getApplicationInfo(SELF_BUNDLE_NAME, appFlags);
      const iconFlags: number = bundleResourceManager.ResourceFlag.GET_RESOURCE_INFO_WITH_DRAWABLE_DESCRIPTOR;
      const resourceInfo = bundleResourceManager.getBundleResourceInfo(
        applicationInfo.name,
        iconFlags,
        applicationInfo.appIndex
      );
      const pixelMap = resourceInfo?.drawableDescriptor?.getPixelMap();
      if (!pixelMap) {
        return null;
      }
      return await this.isPixelMapValid(pixelMap) ? pixelMap : null;
    } catch (e) {
      console.warn(`[AgentController] load self bundle icon failed: ${String(e)}`);
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
      if (!await this.isPixelMapValid(icon)) {
        return;
      }
      this.liveViewAppName = appName;
      this.liveViewIcon = icon;
    } catch (e) {
      console.warn(`[AgentController] load app icon failed: ${String(e)}`);
      return;
    }
    if (this.liveViewPhase === 'running') {
      await this.publishRunningLiveView();
    }
  }

  private async isPixelMapValid(pixelMap: image.PixelMap): Promise<boolean> {
    try {
      const info: image.ImageInfo = await pixelMap.getImageInfo();
      return info.size.width > 0 && info.size.height > 0;
    } catch (_e) {
      return false;
    }
  }

  private bindLiveViewCancelListener(): void {
    if (this.liveViewCancelBound) {
      return;
    }
    this.liveViewController.setCancelListener((notificationId: number, reason?: number) => {
      this.handleLiveViewCanceled(notificationId, reason);
    });
    this.liveViewCancelBound = true;
  }

  private handleLiveViewCanceled(notificationId: number, _reason?: number): void {
    if (notificationId !== this.liveViewNotificationId) {
      return;
    }
    if (this.liveViewSelfStopping) {
      return;
    }
    const snapshot = this.taskManager.getSnapshot();
    if (snapshot.status === 'running' || snapshot.status === 'waiting_user') {
      this.stopTask('liveview_closed');
    }
  }

  private applyEfficiencyResourcesBestEffort(): void {
    if (this.efficiencyApplied) {
      return;
    }
    const request: backgroundTaskManager.EfficiencyResourcesRequest = {
      resourceTypes: backgroundTaskManager.ResourceType.CPU,
      isApply: true,
      timeOut: EFFICIENCY_RESOURCE_TIMEOUT_MS,
      isPersist: true,
      isProcess: true,
      reason: EFFICIENCY_RESOURCE_REASON,
    };
    try {
      backgroundTaskManager.applyEfficiencyResources(request);
      this.efficiencyApplied = true;
      hilog.info(
        HILOG_DOMAIN,
        HILOG_TAG,
        'Efficiency resources applied. type=CPU persist=true timeout=%{public}d',
        EFFICIENCY_RESOURCE_TIMEOUT_MS
      );
    } catch (e) {
      const err = e as BusinessError;
      hilog.warn(
        HILOG_DOMAIN,
        HILOG_TAG,
        'applyEfficiencyResources failed. code=%{public}s msg=%{public}s',
        String(err.code),
        err.message ?? String(e)
      );
    }
  }

  private releaseEfficiencyResourcesBestEffort(): void {
    if (!this.efficiencyApplied) {
      return;
    }
    const request: backgroundTaskManager.EfficiencyResourcesRequest = {
      resourceTypes: backgroundTaskManager.ResourceType.CPU,
      isApply: false,
      timeOut: EFFICIENCY_RESOURCE_TIMEOUT_MS,
      isPersist: true,
      isProcess: true,
      reason: EFFICIENCY_RESOURCE_REASON,
    };
    try {
      backgroundTaskManager.applyEfficiencyResources(request);
      hilog.info(HILOG_DOMAIN, HILOG_TAG, 'Efficiency resources released. type=CPU');
    } catch (e) {
      const err = e as BusinessError;
      hilog.warn(
        HILOG_DOMAIN,
        HILOG_TAG,
        'releaseEfficiencyResources failed. code=%{public}s msg=%{public}s',
        String(err.code),
        err.message ?? String(e)
      );
    } finally {
      this.efficiencyApplied = false;
    }
  }

  releaseEfficiencyResourcesOnExit(): void {
    this.releaseEfficiencyResourcesBestEffort();
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
    const space = this.agentSpace;
    if (!space) {
      return;
    }
    await space.sendViewerEvent(event);
  }

  async pullViewerEffectsJson(lastSequence: number): Promise<string> {
    const space = this.agentSpace;
    if (!space) {
      return '';
    }
    const batch: AgentSpaceViewerEffectBatch | null = await space.pullViewerEffects(lastSequence);
    if (!batch) {
      return '';
    }
    return JSON.stringify(batch);
  }

  private async executeAction(action: AgentAction): Promise<string> {
    const result = await this.requireToolKit().executeAction(action);
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

  private requireToolKit(): ToolKit {
    if (!this.toolKit) {
      this.createAgentSpaceInternal();
    }
    const toolKit = this.toolKit;
    if (!toolKit) {
      throw new Error('toolkit_not_ready');
    }
    return toolKit;
  }

  private getAgentProxy(): AgentProxy {
    if (!this.agentProxy) {
      this.agentProxy = new AgentProxy(DEFAULT_CLOUD_ENDPOINT);
    }
    return this.agentProxy;
  }

  private async resetAgentSpaceForNewTask(): Promise<void> {
    await this.runAgentSpaceOp(async () => {
      await this.destroyAgentSpaceInternal();
      this.createAgentSpaceInternal();
    });
  }

  private runAgentSpaceOp(op: () => Promise<void>): Promise<void> {
    const next = this.agentSpaceOp.then(op, op);
    this.agentSpaceOp = next.catch(() => {
      // keep chain alive
    });
    return next;
  }

  private createAgentSpaceInternal(): void {
    const invoker = this.startAbilityInvoker;
    if (!invoker) {
      throw new Error('Service startAbility not initialized');
    }
    const space = new AgentSpace(invoker);
    this.agentSpace = space;
    this.toolKit = new ToolKit(space);
  }

  private async destroyAgentSpaceInternal(): Promise<void> {
    this.pendingAgentSpaceDestroy = false;
    const space = this.agentSpace;
    this.agentSpace = null;
    this.toolKit = null;
    this.lastScreenshotBase64 = '';
    if (!space) {
      return;
    }
    try {
      await space.destroy();
    } catch (e) {
      hilog.warn(HILOG_DOMAIN, HILOG_TAG, 'destroyAgentSpace failed: %{public}s', String(e));
    }
  }
}

class AgentControllerTaskListener implements TaskManagerListener {
  private readonly owner: AgentController;

  constructor(owner: AgentController) {
    this.owner = owner;
  }

  onStatusChanged(snapshot: TaskSnapshot): void {
    void this.owner.handleTaskStatusChanged(snapshot);
  }

  onAction(action: AgentAction, description: string): void {
    void this.owner.handleTaskAction(action, description);
  }
}
