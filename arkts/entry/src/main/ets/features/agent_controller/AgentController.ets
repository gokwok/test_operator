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
import { VisiblePageContentA11yTree } from './state_collector/a11y_tree/VisiblePageContentA11yTree';
import { ToolKit } from './toolkit/ToolKit';
import { ToolCallDispatcher } from './tool_call/ToolCallDispatcher';
import { AgentEventHub } from './events/AgentEventHub';
import { DeviceToolsEndpoint } from './mcp/DeviceToolsEndpoint';
import type { DeviceToolsConfig, DeviceToolsEndpointObserver, McpStatus } from './mcp/DeviceToolsEndpoint';
import type { McpToolCall } from './mcp/McpProtocol';
import type { ToolCallResult } from './tool_call/ToolCallDispatcher';
import { A2AClient, A2AConfiguration, A2AFile, A2AFilePart, A2AMessage, A2AMessagePart, A2AMetadata, A2ARequest, A2ATextPart } from './a2a/A2AClient';
import type { A2AStreamListener } from './a2a/A2AStreamSession';
import { A2AStreamSession } from './a2a/A2AStreamSession';
import type {
  AppTargetDescriptor,
  TaskEventBatch,
  TaskEventType,
  TaskFlowKind,
  TaskSnapshot,
  TaskStartRequest,
  TaskStatus,
  UserReply
} from './types';
import { TaskManager, TaskManagerListener } from './task_manager/TaskManager';
import { TaskHistoryRecorder } from './task_manager/TaskHistoryRecorder';
import { LiveViewController, LiveViewContents, LiveViewType } from './live_view/LiveViewController';
import { LiveViewPresenter, LiveViewPresenterContext } from './live_view/LiveViewPresenter';
import type { AgentSettingsData } from '../persistence/settings/AgentSettings';
import { AgentSettings } from '../persistence/settings/AgentSettings';
import { AppState } from './state_collector/AppState';
import { ForegroundAppState } from './state_collector/ForegroundAppState';
import { TaskHistoryStore } from '../persistence/history/TaskHistoryStore';
import type { HistoryStepMeta, HistoryTaskMeta, HistoryTaskStatus } from '../persistence/history/TaskHistoryStore';
import type { ActionExecutionObserver, ActionExpectedResult, ActionSequenceStep, UiTreeExpectedTarget } from './execution/ActionExecutionEngine';
import { ActionExecutionEngine } from './execution/ActionExecutionEngine';

const DEFAULT_CLOUD_ENDPOINT: string = 'http://114.55.173.20:8005';
const FLOAT_WINDOW_ABILITY_NAME: string = 'FloatWindowAbility';
const VIEWER_ABILITY_NAME: string = 'AgentSpaceViewerAbility';
const SELF_BUNDLE_NAME: string = 'com.huawei.aios.agent.operation';
const LIVE_VIEW_AUTO_STOP_DELAY_MS: number = 30000;
const EFFICIENCY_RESOURCE_TIMEOUT_MS: number = 60 * 60 * 1000;
const EFFICIENCY_RESOURCE_REASON: string = 'agent_task';
const HILOG_DOMAIN: number = 0x0000;
const HILOG_TAG: string = 'AgentController';

interface DebugToolCallSummary {
  name: string;
  status: string;
  error: string;
  screenshot_len: number;
}

interface ErrorPayload {
  error: string;
}

interface SseChunkLogPayload {
  len: number;
  sample: string;
}

interface A2AStreamStartPayload {
  context_id: string;
  task_id: string;
}

interface ToolExecEventPayload {
  call_id: string;
  tool_name: string;
  phase: string;
  elapsed_ms?: number;
  status?: string;
  error?: string;
}

interface A2AStreamArtifact {
  artifactId?: string;
  parts?: Array<A2AStreamArtifactPart>;
}

interface A2AStreamArtifactUpdate {
  artifact?: A2AStreamArtifact;
  artifacts?: Array<A2AStreamArtifact>;
}

interface A2AStreamArtifactPart {
  kind?: string;
  data?: Object;
  text?: string;
}

interface A2AStreamStatusMessagePart {
  text?: string;
}

interface A2AStreamStatusMessageMetadataActionParams {
  content?: string;
  options?: Array<string>;
}

interface A2AStreamStatusMessageMetadataAction {
  name?: string;
  parameters?: A2AStreamStatusMessageMetadataActionParams;
}

interface A2AStreamStatusMessageMetadata {
  action?: A2AStreamStatusMessageMetadataAction;
  options?: Array<string>;
}

interface A2AStreamStatusMessage {
  parts?: Array<A2AStreamStatusMessagePart>;
  metadata?: A2AStreamStatusMessageMetadata;
}

interface A2ATaskPayload {
  id?: string;
}

interface A2AStreamEnvelope {
  task?: A2ATaskPayload;
}

class A2AStreamListenerImpl implements A2AStreamListener {
  private readonly onEventFn: (payload: Object) => void;
  private readonly onErrorFn: (message: string) => void;
  private readonly onCompleteFn: () => void;
  private readonly onChunkFn: (text: string) => void;

  constructor(
    onEventFn: (payload: Object) => void,
    onErrorFn: (message: string) => void,
    onCompleteFn: () => void,
    onChunkFn?: (text: string) => void
  ) {
    this.onEventFn = onEventFn;
    this.onErrorFn = onErrorFn;
    this.onCompleteFn = onCompleteFn;
    this.onChunkFn = onChunkFn ?? (() => {});
  }

  onEvent(payload: Object): void {
    this.onEventFn(payload);
  }

  onError(message: string): void {
    this.onErrorFn(message);
  }

  onComplete(): void {
    this.onCompleteFn();
  }

  onChunk(text: string): void {
    this.onChunkFn(text);
  }
}

interface A2AStreamStatusUpdate {
  state?: string;
  message?: A2AStreamStatusMessage;
}

interface A2AStreamUpdate {
  statusUpdate?: A2AStreamStatusUpdate;
  artifactUpdate?: A2AStreamArtifactUpdate;
  artifacts?: Array<A2AStreamArtifact>;
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

export class AgentController {
  private static instance: AgentController | null = null;
  private ctx: common.Context | null = null;
  private startAbilityInvoker: StartAbilityInvoker | null = null;
  private readonly createdAtMs: number = Date.now();
  private taskManager: TaskManager = new TaskManager();
  private agentSpace: AgentSpace | null = null;
  private toolKit: ToolKit | null = null;
  private deviceToolsEndpoint: DeviceToolsEndpoint | null = null;
  private deviceToolsToolKit: ToolKit | null = null;
  private agentProxy: AgentProxy | null = null;
  private a2aClient: A2AClient | null = null;
  private a2aStream: A2AStreamSession | null = null;
  private a2aEventHub: AgentEventHub = new AgentEventHub();
  private a2aTaskId: string = '';
  private a2aActive: boolean = false;
  private a2aSnapshot: TaskSnapshot = {
    sessionId: '',
    status: 'idle',
    stepIndex: 0,
    lastDescription: '',
    lastActionName: '',
    lastActionJson: '',
    lastResultStatus: '',
    lastError: '',
    installedAppsSent: false,
  };
  private replaySnapshot: TaskSnapshot = {
    sessionId: '',
    status: 'idle',
    stepIndex: 0,
    lastDescription: '',
    lastActionName: '',
    lastActionJson: '',
    lastResultStatus: '',
    lastError: '',
    installedAppsSent: false,
  };
  private appTargetMap: Map<string, AppTarget> = new Map<string, AppTarget>();
  private lastLaunchTarget: AppTarget | null = null;
  private lastScreenshotBase64: string = '';
  private lastLongScreenshotBase64: string = '';
  private lastA11yTreeList: string = '';
  private lastVisiblePageContent: string = '';
  private lastPageContent: string = '';
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
  private viewerOpening: boolean = false;
  private pendingAgentSpaceDestroy: boolean = false;
  private mcpLogLines: Array<string> = [];
  private mcpSessionId: string = '';
  private historyStore: TaskHistoryStore = TaskHistoryStore.getInstance();
  private a2aHistoryRecorder: TaskHistoryRecorder = new TaskHistoryRecorder(TaskHistoryStore.getInstance());
  private a2aHistoryTaskId: string = '';
  private a2aHistoryStepIndex: number = 0;
  private agentSpaceOp: Promise<void> = Promise.resolve();
  private replayActive: boolean = false;

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
    const snapshot = this.resolveActiveSnapshot();
    snapshot.flow = this.resolveActiveFlowKind();
    const appLaunched = this.agentSpace ? this.agentSpace.isAppLaunched() : false;
    snapshot.appLaunched = appLaunched;
    return JSON.stringify(snapshot);
  }

  private resolveActiveFlowKind(): TaskFlowKind {
    if (this.a2aSnapshot.status !== 'idle' || this.a2aActive || this.a2aTaskId.trim().length > 0) {
      return 'a2a';
    }
    const legacySnapshot = this.taskManager.getSnapshot();
    if (legacySnapshot.status !== 'idle') {
      return 'legacy';
    }
    if (this.replaySnapshot.status !== 'idle') {
      return 'replay';
    }
    return 'legacy';
  }

  private resolveActiveSnapshot(): TaskSnapshot {
    if (this.a2aSnapshot.status !== 'idle' || this.a2aActive || this.a2aTaskId.trim().length > 0) {
      return this.a2aSnapshot;
    }
    const legacySnapshot = this.taskManager.getSnapshot();
    if (legacySnapshot.status !== 'idle') {
      return legacySnapshot;
    }
    if (this.replaySnapshot.status !== 'idle') {
      return this.replaySnapshot;
    }
    return legacySnapshot;
  }

  private isA2AFlowActive(): boolean {
    if (this.a2aActive) {
      return true;
    }
    const status = this.a2aSnapshot.status;
    return status === 'running' || status === 'waiting_user' || status === 'stopping';
  }

  private stopA2AFlow(reason: string): void {
    try {
      this.a2aStopStream();
    } catch (e) {
      const meta: ErrorPayload = { error: String(e) };
      const payload = JSON.stringify(meta);
      this.a2aEventHub.push('error', 'stop_stream_failed', payload);
    }

    this.a2aSnapshot.status = 'stopping';
    this.a2aSnapshot.lastError = reason.trim();
    this.finalizeA2AHistoryTask('stopped', this.a2aSnapshot.lastError);
    void this.publishStoppedLiveView(reason.trim());
    this.releaseEfficiencyResourcesBestEffort();

    const endpoint = this.deviceToolsEndpoint;
    if (endpoint) {
      void endpoint.disconnect();
      this.mcpSessionId = '';
    }

    if (this.agentSpace) {
      if (this.viewerActive) {
        this.pendingAgentSpaceDestroy = true;
      } else {
        this.pendingAgentSpaceDestroy = false;
        void this.runAgentSpaceOp(async () => {
          await this.destroyAgentSpaceInternal();
        });
      }
    }
  }

  private stopA2AForNewTask(): void {
    const status = this.a2aSnapshot.status;
    if ((status === 'running' || status === 'waiting_user') && this.a2aHistoryTaskId.trim().length > 0) {
      this.finalizeA2AHistoryTask('stopped', 'replaced_by_new_task');
    }
    const session = this.a2aStream;
    if (session && session.isActive()) {
      try {
        session.stop();
      } catch (_e) {
        // ignore
      }
    }
    this.a2aActive = false;
    this.a2aTaskId = '';
  }

  private startA2AHistoryTask(taskText: string, a2aBaseUrl: string, contextId: string, waitMs: number): void {
    this.a2aHistoryTaskId = this.a2aHistoryRecorder.createTaskId();
    this.a2aHistoryStepIndex = 0;
    this.a2aHistoryRecorder.recordTaskStart(
      this.a2aHistoryTaskId,
      taskText,
      a2aBaseUrl.trim(),
      contextId.trim(),
      waitMs
    );
  }

  private finalizeA2AHistoryTask(status: HistoryTaskStatus, detail: string): void {
    const taskId = this.a2aHistoryTaskId.trim();
    if (taskId.length === 0) {
      return;
    }
    this.a2aHistoryRecorder.finalizeTask(taskId, status, detail);
  }

  private generateA2AContextId(deviceId: string): string {
    const base = deviceId.trim().length > 0 ? deviceId.trim() : 'device';
    const rand = Math.floor(Math.random() * 1_000_000);
    return `${base}_${Date.now()}_${String(rand).padStart(6, '0')}`;
  }

  private recordA2AToolHistory(call: McpToolCall, result: ToolCallResult, elapsedMs: number): void {
    const taskId = this.a2aHistoryTaskId.trim();
    if (taskId.length === 0) {
      return;
    }
    const a2aSession = this.a2aSnapshot.sessionId.trim();
    const mcpSession = this.mcpSessionId.trim();
    if (a2aSession.length > 0 && mcpSession.length > 0 && a2aSession !== mcpSession) {
      return;
    }

    this.a2aHistoryStepIndex += 1;
    const stepIndex = this.a2aHistoryStepIndex;
    this.a2aSnapshot.stepIndex = stepIndex;

    const description = this.a2aSnapshot.lastDescription.trim();
    const resolvedDescription = description.length > 0 ? description : call.name.trim();
    const action: AgentAction = result.action ?? { name: call.name.trim().toLowerCase() };
    this.a2aHistoryRecorder.recordStep(
      taskId,
      stepIndex,
      resolvedDescription,
      action,
      result.screenshot_b64,
      result.uiTargetText,
      result.uiTargetIndex
    );

    const mapped: ActionResult = { status: result.status === 'SUCCESS' ? 'SUCCESS' : 'FAILED' };
    const err = result.error.trim();
    if (err.length > 0) {
      mapped.error = err;
    }
    if (elapsedMs > 0) {
      mapped.output = `elapsed_ms=${elapsedMs}`;
    }
    this.a2aHistoryRecorder.recordStepResult(taskId, stepIndex, mapped);
  }

  private resetA2ASnapshotForStream(request: A2ARequest): void {
    const sessionId = request.message.contextId.trim();
    const metadata = request.metadata;
    const installedApps = metadata ? metadata.installed_apps : undefined;
    const installedAppsSent = Array.isArray(installedApps) && installedApps.length > 0;

    const resumeTaskId = request.message.taskId;
    const isResume = typeof resumeTaskId === 'string' && resumeTaskId.trim().length > 0;
    const sameSession = sessionId.length > 0 && sessionId === this.a2aSnapshot.sessionId;
    if (isResume && sameSession && this.a2aSnapshot.status !== 'idle') {
      this.a2aSnapshot.status = 'running';
      this.a2aSnapshot.lastError = '';
      this.a2aSnapshot.lastResultStatus = '';
      if (installedAppsSent) {
        this.a2aSnapshot.installedAppsSent = true;
      }
      return;
    }

    const snapshot: TaskSnapshot = {
      sessionId,
      status: 'running',
      stepIndex: 0,
      lastDescription: '',
      lastActionName: '',
      lastActionJson: '',
      lastResultStatus: '',
      lastError: '',
      installedAppsSent,
    };
    this.a2aSnapshot = snapshot;
  }

  private resetA2AStateForLegacy(): void {
    const status = this.a2aSnapshot.status;
    if ((status === 'running' || status === 'waiting_user') && this.a2aHistoryTaskId.trim().length > 0) {
      this.finalizeA2AHistoryTask('stopped', 'replaced_by_legacy_task');
    }
    const session = this.a2aStream;
    if (session && session.isActive()) {
      try {
        session.stop();
      } catch (_e) {
        // ignore
      }
    }
    this.a2aActive = false;
    this.a2aTaskId = '';
    const snapshot: TaskSnapshot = {
      sessionId: '',
      status: 'idle',
      stepIndex: 0,
      lastDescription: '',
      lastActionName: '',
      lastActionJson: '',
      lastResultStatus: '',
      lastError: '',
      installedAppsSent: false,
    };
    this.a2aSnapshot = snapshot;
  }

  getContext(): common.Context | null {
    return this.ctx;
  }

  async startTask(request: TaskStartRequest): Promise<void> {
    this.ensureListenerBound();
    this.bindLiveViewCancelListener();
    this.resetA2AStateForLegacy();
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
    this.applySettings(settings);
    this.applyEfficiencyResourcesBestEffort();
    this.bindLiveViewCancelListener();
    this.stopA2AForNewTask();
    await this.resetAgentSpaceForNewTask();
    const contextId = this.generateA2AContextId(settings.deviceId);
    this.startA2AHistoryTask(trimmed, settings.a2aBaseUrl, contextId, settings.waitMs);
    await this.ensureMcpConnected(settings, contextId);

    const a2aBaseUrl = settings.a2aBaseUrl.trim();
    if (a2aBaseUrl.length === 0) {
      throw new Error('a2a_base_url_empty');
    }
    this.liveViewTaskText = trimmed;
    this.liveViewPhase = 'running';
    this.liveViewStarted = false;
    this.liveViewAppName = '';
    this.liveViewIcon = null;
    this.liveViewNotificationId = this.allocateLiveViewId();
    void this.publishRunningLiveView();

    await this.startTaskWithA2A(trimmed, settings, contextId);
    await this.hideFloatWindow();
  }

  private async startTaskWithDefaultsLegacy(
    taskText: string,
    settings: AgentSettingsData,
    ctx: common.Context
  ): Promise<void> {
    const installedApps = await this.safeListAppNames(ctx);
    const appTargets = await this.safeListAppTargets(ctx);
    const request: TaskStartRequest = {
      sessionId: '',
      taskText,
      installedApps,
      appTargets,
      waitMs: settings.waitMs,
    };
    if (settings.cloudEndpoint.length > 0) {
      this.setCloudEndpoint(settings.cloudEndpoint);
    }
    await this.startTask(request);
    await this.hideFloatWindow();
  }

  private async startTaskWithA2A(taskText: string, settings: AgentSettingsData, contextId: string): Promise<void> {
    const request = await this.buildA2ARequest(taskText, '', true, contextId);
    this.startA2AStreamInternal(settings.a2aBaseUrl, request);
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
    if (this.isA2AFlowActive()) {
      this.stopA2AFlow(reason);
      return;
    }
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
    if (this.a2aTaskId.length > 0 && this.a2aSnapshot.status === 'waiting_user') {
      void this.resumeA2AWithReply(reply);
      return;
    }
    this.taskManager.submitUserReply(reply);
  }

  private async resumeA2AWithReply(reply: UserReply): Promise<void> {
    try {
      await this.a2aResumeStream(this.a2aTaskId, reply.content);
    } catch (e) {
      const meta: ErrorPayload = { error: String(e) };
      const payload = JSON.stringify(meta);
      this.a2aEventHub.push('error', 'resume_failed', payload);
    }
  }

  private async submitA2AUserNote(content: string): Promise<void> {
    const trimmed = content.trim();
    if (trimmed.length === 0) {
      return;
    }
    // A2A does not support injecting user_note mid-run (SESSION_BUSY); keep it observable for now.
    this.a2aEventHub.push('log', 'user_note', trimmed);
  }

  requestTakeOver(content: string): void {
    if (this.isA2AFlowActive()) {
      this.stopA2AFlow('take_over');
      return;
    }
    this.taskManager.requestTakeOver(content);
  }

  requestSupplement(content: string): void {
    if (this.isA2AFlowActive()) {
      // UI already enters supplement mode; server-side "pause" is not supported in A2A.
      this.a2aEventHub.push('log', 'supplement_requested');
      return;
    }
    this.taskManager.requestSupplement(content);
  }

  submitUserNote(content: string): void {
    if (this.isA2AFlowActive()) {
      void this.submitA2AUserNote(content);
      return;
    }
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

  async prepareA11yTreeList(windowId: number = 0): Promise<string> {
    try {
      const toolKit = this.requireToolKit();
      const runtime = await toolKit.ensureVts();
      const wid = Math.max(0, Math.floor(windowId));
      const dump = await toolKit.getVisiblePageContent(wid);
      this.lastVisiblePageContent = dump;
      this.logDebugLarge('debug_a11y_raw', dump);
      const list = VisiblePageContentA11yTree.buildListFromDump(dump, runtime.widthPx, runtime.heightPx);
      this.lastA11yTreeList = JSON.stringify(list);
      this.logDebugLarge('debug_a11y_list', this.lastA11yTreeList);
      return String(this.lastA11yTreeList.length);
    } catch (e) {
      this.lastA11yTreeList = '';
      return `ERR_${String(e)}`;
    }
  }

  private logDebugLarge(label: string, payload: string): void {
    const trimmedLabel = label.trim();
    if (trimmedLabel.length === 0) {
      return;
    }
    const normalized = this.normalizeDebugPayload(payload);
    const total = normalized.length;
    hilog.info(HILOG_DOMAIN, HILOG_TAG, '%{public}s len=%{public}d', trimmedLabel, total);
    if (total === 0) {
      return;
    }
    const chunkSize = 900;
    const maxChars = 24000;
    const limit = Math.min(total, maxChars);
    let offset = 0;
    let part = 0;
    while (offset < limit) {
      const end = Math.min(limit, offset + chunkSize);
      const chunk = normalized.slice(offset, end);
      hilog.info(HILOG_DOMAIN, HILOG_TAG, '%{public}s[%{public}d] %{public}s', trimmedLabel, part, chunk);
      offset = end;
      part += 1;
    }
    if (total > limit) {
      hilog.info(HILOG_DOMAIN, HILOG_TAG, '%{public}s truncated=%{public}d', trimmedLabel, total - limit);
    }
  }

  private normalizeDebugPayload(payload: string): string {
    if (payload.length === 0) {
      return '';
    }
    let normalized = payload;
    normalized = normalized.split('\r').join('\\r');
    normalized = normalized.split('\n').join('\\n');
    return normalized;
  }

  readA11yTreeListChunk(offset: number, size: number): string {
    const off = Math.max(0, Math.floor(offset));
    const len = this.lastA11yTreeList.length;
    if (off >= len) {
      return '';
    }
    const sz = Math.max(0, Math.floor(size));
    if (sz <= 0) {
      return '';
    }
    return this.lastA11yTreeList.substring(off, Math.min(len, off + sz));
  }

  async prepareVisiblePageContent(windowId: number = 0): Promise<string> {
    const toolKit = this.requireToolKit();
    await toolKit.ensureVts();
    const wid = Math.max(0, Math.floor(windowId));
    this.lastVisiblePageContent = await toolKit.getVisiblePageContent(wid);
    return String(this.lastVisiblePageContent.length);
  }

  readVisiblePageContentChunk(offset: number, size: number): string {
    const off = Math.max(0, Math.floor(offset));
    const len = this.lastVisiblePageContent.length;
    if (off >= len) {
      return '';
    }
    const sz = Math.max(0, Math.floor(size));
    if (sz <= 0) {
      return '';
    }
    return this.lastVisiblePageContent.substring(off, Math.min(len, off + sz));
  }

  async preparePageContent(windowId: number = 0): Promise<string> {
    const toolKit = this.requireToolKit();
    await toolKit.ensureVts();
    const wid = Math.max(0, Math.floor(windowId));
    this.lastPageContent = await toolKit.getPageContent(wid);
    return String(this.lastPageContent.length);
  }

  readPageContentChunk(offset: number, size: number): string {
    const off = Math.max(0, Math.floor(offset));
    const len = this.lastPageContent.length;
    if (off >= len) {
      return '';
    }
    const sz = Math.max(0, Math.floor(size));
    if (sz <= 0) {
      return '';
    }
    return this.lastPageContent.substring(off, Math.min(len, off + sz));
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

  async launchAppByName(appName: string): Promise<void> {
    const trimmed = appName.trim();
    if (trimmed.length === 0) {
      throw new Error('app_name_empty');
    }
    const ctx = this.ctx;
    if (!ctx) {
      throw new Error('context_missing');
    }
    const targets = await this.safeListAppTargets(ctx);
    this.appTargetMap = this.buildAppTargetMap(targets);
    this.requireToolKit().setAppTargets(targets);

    const target = this.resolveAppTarget(trimmed);
    if (!target) {
      throw new Error('app_not_mapped');
    }
    await this.launchApp(target.bundleName, target.abilityName);
  }

  async launchApp(bundleName: string, abilityName: string): Promise<void> {
    const target: AppTarget = { bundleName, abilityName };
    this.lastLaunchTarget = target;
    await this.requireToolKit().launchApp(target);
    await this.openViewerIfForegroundMatch(target);
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

  async captureLongScreenshotLength(): Promise<string> {
    const base64 = await this.requireToolKit().screenshotLongBase64();
    this.lastLongScreenshotBase64 = base64;
    return String(base64.length);
  }

  readLongScreenshotChunk(offset: number, size: number): string {
    if (this.lastLongScreenshotBase64.length === 0) {
      return '';
    }
    const safeOffset = Math.max(0, Math.floor(offset));
    const safeSize = Math.max(0, Math.floor(size));
    if (safeSize === 0 || safeOffset >= this.lastLongScreenshotBase64.length) {
      return '';
    }
    const end = Math.min(this.lastLongScreenshotBase64.length, safeOffset + safeSize);
    return this.lastLongScreenshotBase64.substring(safeOffset, end);
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
    const snapshot = this.resolveActiveSnapshot();
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

  async debugSimulateToolCall(name: string, argsJson: string): Promise<string> {
    const waitMs = await this.resolveToolWaitMs();
    const dispatcher = new ToolCallDispatcher(this.requireToolKit(), waitMs);
    const result = await dispatcher.execute(name, argsJson);
    const summary: DebugToolCallSummary = {
      name: name.trim(),
      status: result.status,
      error: result.error,
      screenshot_len: result.screenshot_b64.length,
    };
    return JSON.stringify(summary);
  }

  async mcpConnect(): Promise<string> {
    let config: DeviceToolsConfig | null = null;
    try {
      const endpoint = this.requireDeviceToolsEndpoint();
      config = await this.loadDeviceToolsConfig();
      const status = await endpoint.connect(config);
      this.mcpSessionId = config.sessionId.trim();
      return JSON.stringify(status);
    } catch (e) {
      const url = config ? config.wsUrl : '';
      const status: McpStatus = { state: 'error', url, error: String(e) };
      return JSON.stringify(status);
    }
  }

  async mcpDisconnect(): Promise<string> {
    const endpoint = this.deviceToolsEndpoint;
    if (!endpoint) {
      const status: McpStatus = { state: 'idle', url: '', error: '' };
      return JSON.stringify(status);
    }
    const status = await endpoint.disconnect();
    this.mcpSessionId = '';
    return JSON.stringify(status);
  }

  mcpStatus(): string {
    const endpoint = this.deviceToolsEndpoint;
    if (!endpoint) {
      const status: McpStatus = { state: 'idle', url: '', error: '' };
      return JSON.stringify(status);
    }
    const status = endpoint.getStatus();
    return JSON.stringify(status);
  }

  async a2aSendBlocking(prompt: string): Promise<string> {
    const text = prompt.trim();
    if (text.length === 0) {
      throw new Error('prompt_empty');
    }
    const ctx = this.ctx;
    if (!ctx) {
      throw new Error('context_missing');
    }
    const settings = await AgentSettings.load(ctx);
    await this.ensureMcpConnected(settings);
    const request = await this.buildA2ARequest(text, '', true);
    const client = this.getA2AClient(settings.a2aBaseUrl);
    return await client.messageSend(request);
  }

  async a2aResumeBlocking(taskId: string, reply: string): Promise<string> {
    const task = taskId.trim();
    if (task.length === 0) {
      throw new Error('task_id_empty');
    }
    const text = reply.trim();
    if (text.length === 0) {
      throw new Error('reply_empty');
    }
    const ctx = this.ctx;
    if (!ctx) {
      throw new Error('context_missing');
    }
    const settings = await AgentSettings.load(ctx);
    await this.ensureMcpConnected(settings);
    const request = await this.buildA2ARequest(text, task, false);
    const client = this.getA2AClient(settings.a2aBaseUrl);
    return await client.messageSend(request);
  }

  async a2aStartStream(prompt: string, contextIdOverride: string): Promise<string> {
    const text = prompt.trim();
    if (text.length === 0) {
      throw new Error('prompt_empty');
    }
    const ctx = this.ctx;
    if (!ctx) {
      throw new Error('context_missing');
    }
    const settings = await AgentSettings.load(ctx);
    const override = contextIdOverride.trim();
    const contextId = override.length > 0
      ? override
      : (settings.contextId.trim().length > 0 ? settings.contextId.trim() : settings.deviceId.trim());
    await this.ensureMcpConnected(settings, contextId);
    const request = await this.buildA2ARequest(text, '', true, contextIdOverride);
    this.startA2AStreamInternal(settings.a2aBaseUrl, request);
    return 'ok';
  }

  async a2aResumeStream(taskId: string, reply: string): Promise<string> {
    const task = taskId.trim();
    if (task.length === 0) {
      throw new Error('task_id_empty');
    }
    const text = reply.trim();
    if (text.length === 0) {
      throw new Error('reply_empty');
    }
    const ctx = this.ctx;
    if (!ctx) {
      throw new Error('context_missing');
    }
    const settings = await AgentSettings.load(ctx);
    const storedSession = this.a2aSnapshot.sessionId.trim();
    const contextId = storedSession.length > 0
      ? storedSession
      : (settings.contextId.trim().length > 0 ? settings.contextId.trim() : settings.deviceId.trim());
    await this.ensureMcpConnected(settings, contextId);
    const request = await this.buildA2ARequest(text, task, false, contextId);
    this.startA2AStreamInternal(settings.a2aBaseUrl, request);
    return 'ok';
  }

  a2aStopStream(): string {
    if (this.a2aStream) {
      this.a2aStream.stop();
      this.a2aEventHub.push('status', 'stream_stopped');
    }
    this.a2aActive = false;
    return 'ok';
  }

  private startA2AStreamInternal(baseUrl: string, request: A2ARequest): void {
    const session = this.getA2AStreamSession();
    if (session.isActive()) {
      session.stop();
    }
    this.a2aEventHub.reset();
    this.a2aEventHub.push('log', 'stream_started');
    const startMeta: A2AStreamStartPayload = {
      context_id: request.message.contextId,
      task_id: typeof request.message.taskId === 'string' ? request.message.taskId : '',
    };
    this.a2aEventHub.push('log', 'stream_meta', JSON.stringify(startMeta));
    const taskId = request.message.taskId;
    this.a2aTaskId = typeof taskId === 'string' ? taskId.trim() : '';
    this.resetA2ASnapshotForStream(request);
    this.a2aActive = true;
    let lastChunkLog = 0;
    let chunkCount = 0;
    const listener = new A2AStreamListenerImpl(
      (payload: Object) => {
        this.handleA2AStreamUpdate(payload);
      },
      (message: string) => {
        const meta: ErrorPayload = { error: message };
        const payload = JSON.stringify(meta);
        this.a2aEventHub.push('error', 'stream_error', payload);
        this.a2aActive = false;
        this.a2aSnapshot.status = 'failed';
        this.a2aSnapshot.lastError = message;
        void this.handleA2AStatusChanged('failed', message);
      },
      () => {
        this.a2aEventHub.push('status', 'stream_closed');
        this.a2aActive = false;
      },
      (text: string) => {
        chunkCount += 1;
        const now = Date.now();
        if (chunkCount <= 2 || now - lastChunkLog > 1500) {
          lastChunkLog = now;
          const snippet = text.length > 180 ? text.slice(0, 180) : text;
          const meta: SseChunkLogPayload = { len: text.length, sample: snippet };
          const payload = JSON.stringify(meta);
          this.a2aEventHub.push('log', 'sse_chunk', payload);
        }
      }
    );
    session.start(baseUrl, request, listener);
  }

  a2aPullEventsJson(lastSequence: number): string {
    const batch = this.a2aEventHub.pull(lastSequence);
    if (!batch) {
      return '';
    }
    return JSON.stringify(batch);
  }

  async replaySkill(skillName: string): Promise<string> {
    const name = skillName.trim();
    if (name.length === 0) {
      return 'ERR_skill_name_empty';
    }
    if (this.replayActive) {
      return 'ERR_replay_busy';
    }
    if (this.isA2AFlowActive() || this.taskManager.getSnapshot().status === 'running') {
      return 'ERR_task_running';
    }
    const ctx = this.ctx;
    if (!ctx) {
      return 'ERR_context_missing';
    }
    const settings = await AgentSettings.load(ctx);
    this.applySettings(settings);
    this.applyEfficiencyResourcesBestEffort();
    this.ensureListenerBound();
    this.bindLiveViewCancelListener();
    await this.resetAgentSpaceForNewTask();
    try {
      const toolKit = this.requireToolKit();
      const targets = await this.safeListAppTargets(ctx);
      toolKit.setAppTargets(targets);
      this.appTargetMap = this.buildAppTargetMap(targets);
    } catch (e) {
      hilog.warn(HILOG_DOMAIN, HILOG_TAG, 'replaySkill load appTargets failed: %{public}s', String(e));
    }

    const taskId = await this.findLatestTaskIdBySkillName(name);
    if (taskId.length === 0) {
      return 'ERR_skill_not_found';
    }
    const steps = await this.historyStore.listSteps(taskId);
    if (steps.length === 0) {
      return 'ERR_skill_steps_empty';
    }

    this.liveViewTaskText = `回放技能: ${name}`;
    this.liveViewPhase = 'running';
    this.liveViewStarted = false;
    this.liveViewAppName = '';
    this.liveViewIcon = null;
    this.liveViewNotificationId = this.allocateLiveViewId();
    void this.publishRunningLiveView();

    this.replaySnapshot.sessionId = taskId;
    this.replaySnapshot.status = 'running';
    this.replaySnapshot.stepIndex = 0;
    this.replaySnapshot.lastDescription = this.liveViewTaskText;
    this.replaySnapshot.lastActionName = '';
    this.replaySnapshot.lastActionJson = '';
    this.replaySnapshot.lastResultStatus = '';
    this.replaySnapshot.lastError = '';

    this.replayActive = true;
    const delayMs = 1000;
    void this.runReplaySteps(steps, delayMs).finally(() => {
      this.replayActive = false;
    });
    return JSON.stringify({ status: 'started', skill_name: name, taskId });
  }

  async replayTask(taskId: string): Promise<string> {
    const id = taskId.trim();
    if (id.length === 0) {
      return 'ERR_task_id_empty';
    }
    if (this.replayActive) {
      return 'ERR_replay_busy';
    }
    if (this.isA2AFlowActive() || this.taskManager.getSnapshot().status === 'running') {
      return 'ERR_task_running';
    }
    const ctx = this.ctx;
    if (!ctx) {
      return 'ERR_context_missing';
    }
    const settings = await AgentSettings.load(ctx);
    this.applySettings(settings);
    this.applyEfficiencyResourcesBestEffort();
    this.ensureListenerBound();
    this.bindLiveViewCancelListener();
    await this.resetAgentSpaceForNewTask();
    try {
      const toolKit = this.requireToolKit();
      const targets = await this.safeListAppTargets(ctx);
      toolKit.setAppTargets(targets);
      this.appTargetMap = this.buildAppTargetMap(targets);
    } catch (e) {
      hilog.warn(HILOG_DOMAIN, HILOG_TAG, 'replayTask load appTargets failed: %{public}s', String(e));
    }

    this.historyStore.init(ctx);
    const meta = await this.historyStore.getTask(id);
    if (!meta) {
      return 'ERR_task_not_found';
    }
    const steps = await this.historyStore.listSteps(id);
    if (steps.length === 0) {
      return 'ERR_task_steps_empty';
    }

    this.liveViewTaskText = this.buildReplayTitle(meta);
    this.liveViewPhase = 'running';
    this.liveViewStarted = false;
    this.liveViewAppName = '';
    this.liveViewIcon = null;
    this.liveViewNotificationId = this.allocateLiveViewId();
    void this.publishRunningLiveView();

    this.replaySnapshot.sessionId = id;
    this.replaySnapshot.status = 'running';
    this.replaySnapshot.stepIndex = 0;
    this.replaySnapshot.lastDescription = this.liveViewTaskText;
    this.replaySnapshot.lastActionName = '';
    this.replaySnapshot.lastActionJson = '';
    this.replaySnapshot.lastResultStatus = '';
    this.replaySnapshot.lastError = '';

    this.replayActive = true;
    const delayMs = 1000;
    void this.runReplaySteps(steps, delayMs).finally(() => {
      this.replayActive = false;
    });
    return JSON.stringify({ status: 'started', taskId: id, skill_name: meta.skill_name });
  }

  private buildReplayTitle(meta: HistoryTaskMeta): string {
    const name = typeof meta.skill_name === 'string' ? meta.skill_name.trim() : '';
    if (name.length > 0) {
      return `回放技能: ${name}`;
    }
    const taskText = typeof meta.taskText === 'string' ? meta.taskText.trim() : '';
    if (taskText.length > 0) {
      const maxLen = 24;
      const preview = taskText.length > maxLen ? `${taskText.slice(0, maxLen)}...` : taskText;
      return `回放任务: ${preview}`;
    }
    return '回放任务';
  }

  private async runReplaySteps(steps: Array<HistoryStepMeta>, delayMs: number): Promise<void> {
    try {
      const toolKit = this.requireToolKit();
      await toolKit.ensureVts();
      const ordered = steps.slice().sort((a, b) => a.stepIndex - b.stepIndex);
      const plan = this.buildReplayPlan(ordered);
      if (plan.length === 0) {
        throw new Error('empty_replay_plan');
      }
      const engine = new ActionExecutionEngine(toolKit);
      const observer: ActionExecutionObserver = {
        onStepStart: async (item: ActionSequenceStep, position: number): Promise<void> => {
          const desc = item.description.trim();
          if (desc.length > 0) {
            this.replaySnapshot.lastDescription = desc;
          }
          const action = item.action;
          this.replaySnapshot.status = 'running';
          this.replaySnapshot.stepIndex = position + 1;
          this.replaySnapshot.lastActionName = action.name;
          this.replaySnapshot.lastActionJson = JSON.stringify(action);
          this.replaySnapshot.lastResultStatus = '';
          this.replaySnapshot.lastError = '';
          if ((action.name ?? '').trim().toLowerCase() === 'launch_app') {
            const appName = this.resolveAppName(action);
            if (appName.length > 0) {
              await this.handleLaunchAppUiSideEffects(appName);
            }
          }
          hilog.info(HILOG_DOMAIN, HILOG_TAG, 'replay step=%{public}d action=%{public}s', position + 1, action.name);
        },
        onStepComplete: async (_item: ActionSequenceStep, _position: number, result: ActionResult): Promise<void> => {
          this.replaySnapshot.lastResultStatus = result.status;
          const err = (result.error ?? '').trim();
          this.replaySnapshot.lastError = err;
        },
        onWaitExpectedStart: async (_step: ActionSequenceStep, position: number, expected: UiTreeExpectedTarget): Promise<void> => {
          const text = (expected.text ?? '').trim();
          const idx = typeof expected.index === 'number' && Number.isFinite(expected.index) ? Math.max(0, Math.floor(expected.index)) : 0;
          hilog.info(
            HILOG_DOMAIN,
            HILOG_TAG,
            'replay wait_expected_start step=%{public}d text=%{public}s index=%{public}d',
            position + 1,
            text,
            idx
          );
        },
        onWaitExpectedDone: async (_step: ActionSequenceStep, position: number, matched: boolean, elapsedMs: number): Promise<void> => {
          hilog.info(
            HILOG_DOMAIN,
            HILOG_TAG,
            'replay wait_expected_done step=%{public}d matched=%{public}s elapsed_ms=%{public}d',
            position + 1,
            String(matched),
            Math.max(0, Math.floor(elapsedMs))
          );
        },
      };
      await engine.run(plan, observer, { defaultWaitMs: delayMs, expectTimeoutMs: 5000, pollIntervalMs: 200, windowId: 0 });
      this.replaySnapshot.status = 'finished';
      this.replaySnapshot.lastDescription = '回放完成';
      await this.publishFinishedLiveView('回放完成');
    } catch (e) {
      hilog.warn(HILOG_DOMAIN, HILOG_TAG, 'replay failed: %{public}s', String(e));
      this.replaySnapshot.status = 'failed';
      this.replaySnapshot.lastError = String(e);
      this.replaySnapshot.lastDescription = '回放失败';
      await this.publishFailedLiveView(`回放失败: ${String(e)}`);
    }
  }

  private buildReplayPlan(steps: Array<HistoryStepMeta>): Array<ActionSequenceStep> {
    const ordered = steps.slice().sort((a, b) => a.stepIndex - b.stepIndex);
    const plan: Array<ActionSequenceStep> = [];
    for (let i = 0; i < ordered.length; i += 1) {
      const step = ordered[i];
      const action = this.parseHistoryAction(step.actionJson);
      if (!action) {
        const raw = step.actionJson.trim();
        const snippet = raw.length > 120 ? `${raw.slice(0, 120)}...` : raw;
        hilog.warn(HILOG_DOMAIN, HILOG_TAG, 'replay skip step=%{public}d invalid_action=%{public}s', step.stepIndex, snippet);
        continue;
      }
      const description = typeof step.description === 'string' ? step.description : '';
      let expected: UiTreeExpectedTarget | null = null;
      if (i < ordered.length - 1) {
        const next = ordered[i + 1];
        const text = (next.uiTargetText ?? '').trim();
        const rawIndex = next.uiTargetIndex ?? 0;
        const idx = typeof rawIndex === 'number' && Number.isFinite(rawIndex) ? Math.max(0, Math.floor(rawIndex)) : 0;
        if (text.length > 0 || idx > 0) {
          const uiTarget: UiTreeExpectedTarget = { text, index: idx };
          expected = uiTarget;
        }
      }
      let expectedResult: ActionExpectedResult | undefined = undefined;
      if (expected) {
        expectedResult = { uiTarget: expected };
      }
      const item: ActionSequenceStep = { action, description, expected: expectedResult };
      plan.push(item);
    }
    return plan;
  }

  private parseHistoryAction(actionJson: string): AgentAction | null {
    const raw = actionJson.trim();
    if (raw.length === 0) {
      return null;
    }
    try {
      const parsed = JSON.parse(raw) as AgentAction;
      const name = (parsed?.name ?? '').trim();
      if (name.length === 0) {
        return null;
      }
      return { name, parameters: parsed.parameters };
    } catch (_e) {
      return null;
    }
  }

  private async findLatestTaskIdBySkillName(skillName: string): Promise<string> {
    const ctx = this.ctx;
    if (!ctx) {
      return '';
    }
    this.historyStore.init(ctx);
    const tasks = await this.historyStore.listTasks(200);
    let bestTaskId: string = '';
    let bestTs: number = 0;
    for (const task of tasks) {
      const name = typeof task.skill_name === 'string' ? task.skill_name.trim() : '';
      if (name.length === 0 || name !== skillName) {
        continue;
      }
      const ts = task.endAtMs > 0 ? task.endAtMs : task.startAtMs;
      if (ts > bestTs) {
        bestTs = ts;
        bestTaskId = task.taskId;
      }
    }
    return bestTaskId;
  }

  private async sleepMs(ms: number): Promise<void> {
    const delay = Math.max(0, Math.floor(ms));
    await new Promise<void>((resolve) => setTimeout(resolve, delay));
  }

  mcpLogs(): string {
    return JSON.stringify(this.mcpLogLines);
  }

  clearMcpLogs(): void {
    this.mcpLogLines = [];
  }

  handleDeviceToolCallStarted(call: McpToolCall): void {
    this.handleLaunchAppToolCallBestEffort(call, null);
    if (this.a2aHistoryTaskId.trim().length === 0) {
      return;
    }
    const meta: ToolExecEventPayload = {
      call_id: call.call_id,
      tool_name: call.name,
      phase: 'started',
    };
    this.a2aEventHub.push('tool.exec', 'tool.call.started', JSON.stringify(meta));
  }

  handleDeviceToolCallCompleted(call: McpToolCall, result: ToolCallResult, elapsedMs: number): void {
    const toolName = call.name.trim().toLowerCase();
    const hitText = result.uiTargetText.trim();
    const hitIndex = result.uiTargetIndex;
    let shouldLogHit = toolName === 'click' || toolName === 'long_click';
    if (!shouldLogHit && toolName === 'type') {
      const pt = result.action?.parameters?.point;
      shouldLogHit = Array.isArray(pt) && pt.length >= 2;
    }
    if (shouldLogHit) {
      if (hitText.length > 0) {
        hilog.info(
          HILOG_DOMAIN,
          HILOG_TAG,
          'tool_call hit_text tool=%{public}s call_id=%{public}s text=%{public}s index=%{public}d',
          call.name,
          call.call_id,
          hitText,
          hitIndex
        );
      } else {
        hilog.info(
          HILOG_DOMAIN,
          HILOG_TAG,
          'tool_call hit_text_empty tool=%{public}s call_id=%{public}s index=%{public}d',
          call.name,
          call.call_id,
          hitIndex
        );
      }
    }
    this.recordA2AToolHistory(call, result, elapsedMs);
    const meta: ToolExecEventPayload = {
      call_id: call.call_id,
      tool_name: call.name,
      phase: 'completed',
      elapsed_ms: elapsedMs,
      status: result.status,
      error: result.error,
    };
    this.a2aEventHub.push('tool.exec', 'tool.call.completed', JSON.stringify(meta));
  }

  async actionPressBackspace(): Promise<string> {
    const action: AgentAction = { name: 'press_backspace' };
    return await this.executeAction(action);
  }

  async actionPressKey(key: string): Promise<string> {
    const params: ActionParameters = { key };
    const action: AgentAction = { name: 'press_key', parameters: params };
    return await this.executeAction(action);
  }

  async actionPressComboKey(keys: Array<string>): Promise<string> {
    const params: ActionParameters = { keys };
    const action: AgentAction = { name: 'press_combo_key', parameters: params };
    return await this.executeAction(action);
  }

  async actionRequestFocus(): Promise<string> {
    const action: AgentAction = { name: 'request_focus' };
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
    const name = action.name.trim().toLowerCase();
    if (name === 'launch_app') {
      const appName = this.resolveAppName(action);
      if (appName.length > 0) {
        await this.handleLaunchAppUiSideEffects(appName);
      }
      return;
    }
    if (!this.liveViewEnabled) {
      return;
    }
    if (name === 'call_user' || name === 'interact' || name === 'take_over') {
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

  private handleLaunchAppToolCallBestEffort(call: McpToolCall, result: ToolCallResult | null): void {
    const name = call.name.trim().toLowerCase();
    if (name !== 'launch_app') {
      return;
    }
    if (result && result.status !== 'SUCCESS') {
      return;
    }
    const appName = this.resolveToolCallAppName(call);
    if (appName.length === 0) {
      return;
    }
    void this.handleLaunchAppUiSideEffects(appName);
  }

  private resolveToolCallAppName(call: McpToolCall): string {
    const args = call.arguments;
    if (!args) {
      return '';
    }
    const raw = Reflect.get(args, 'app') as Object | null | undefined;
    if (typeof raw !== 'string') {
      return '';
    }
    return raw.trim();
  }

  private async handleLaunchAppUiSideEffects(appName: string): Promise<void> {
    if (!this.shouldApplyLaunchAppUiSideEffects()) {
      return;
    }
    const trimmed = appName.trim();
    if (trimmed.length === 0) {
      return;
    }
    hilog.info(HILOG_DOMAIN, HILOG_TAG, 'launch_app ui: %{public}s', trimmed);
    const target = this.resolveAppTarget(trimmed);
    if (!target) {
      await this.updateLiveViewIcon(trimmed);
      return;
    }
    await this.updateLiveViewIcon(target.bundleName);
    await this.openViewerIfForegroundMatch(target);
  }

  private shouldApplyLaunchAppUiSideEffects(): boolean {
    if (!this.liveViewEnabled) {
      return false;
    }
    if (this.replayActive) {
      return true;
    }
    if (this.isA2AFlowActive()) {
      return true;
    }
    const status = this.taskManager.getSnapshot().status;
    return status === 'running' || status === 'waiting_user';
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

  private async openViewerIfForegroundMatch(target: AppTarget): Promise<void> {
    if (this.viewerActive || this.viewerOpening) {
      return;
    }
    const foregroundBundle = await ForegroundAppState.getForegroundAppBundleName();
    if (foregroundBundle.length === 0) {
      return;
    }
    if (foregroundBundle !== target.bundleName) {
      return;
    }
    const invoker = this.startAbilityInvoker;
    if (!invoker) {
      return;
    }
    const want: Want = {
      bundleName: SELF_BUNDLE_NAME,
      abilityName: VIEWER_ABILITY_NAME,
    };
    this.viewerOpening = true;
    try {
      await invoker(want);
    } catch (e) {
      hilog.warn(HILOG_DOMAIN, HILOG_TAG, 'open viewer failed: %{public}s', String(e));
    } finally {
      this.viewerOpening = false;
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
    const snapshot = this.resolveActiveSnapshot();
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

  private requireDeviceToolsEndpoint(): DeviceToolsEndpoint {
    const toolKit = this.requireToolKit();
    if (!this.deviceToolsEndpoint || this.deviceToolsToolKit !== toolKit) {
      const existing = this.deviceToolsEndpoint;
      if (existing) {
        void existing.disconnect();
      }
      this.deviceToolsEndpoint = new DeviceToolsEndpoint(toolKit, (line: string) => {
        this.appendMcpLog(line);
      }, new AgentControllerDeviceToolsObserver(this));
      this.deviceToolsToolKit = toolKit;
    }
    return this.deviceToolsEndpoint;
  }

  private getA2AClient(baseUrl: string): A2AClient {
    if (!this.a2aClient) {
      this.a2aClient = new A2AClient(baseUrl);
      return this.a2aClient;
    }
    this.a2aClient.setBaseUrl(baseUrl);
    return this.a2aClient;
  }

  private getA2AStreamSession(): A2AStreamSession {
    if (!this.a2aStream) {
      this.a2aStream = new A2AStreamSession();
    }
    return this.a2aStream;
  }

  private handleA2AStreamUpdate(payload: Object): void {
    const raw = this.safeStringify(payload);
    const update = payload as A2AStreamUpdate;
    let handled = false;

    const task = (payload as A2AStreamEnvelope).task;
    if (task && typeof task.id === 'string' && task.id.length > 0) {
      this.a2aTaskId = task.id;
      this.a2aActive = true;
    }

    const statusUpdate = update.statusUpdate;
    if (statusUpdate) {
      const state = typeof statusUpdate.state === 'string' ? statusUpdate.state : '';
      const messageText = this.readStatusMessage(statusUpdate);
      const eventType = state === 'input-required' ? 'input_required' : 'status';
      const message = messageText.length > 0 ? messageText : state;
      this.a2aEventHub.push(eventType, message.length > 0 ? message : 'statusUpdate', raw);
      this.applyA2AStatusUpdate(statusUpdate);
      if (state === 'completed' || state === 'failed') {
        this.a2aActive = false;
      }
      handled = true;
    }

    const artifactUpdate = update.artifactUpdate;
    if (artifactUpdate) {
      const artifactId = this.readArtifactId(update);
      let eventType: TaskEventType = 'delta';
      if (artifactId === 'thinkflow.delta' || artifactId === 'assistant.text') {
        eventType = 'assistant.delta';
      } else if (artifactId === 'tool.trace') {
        eventType = 'tool.trace';
      }
      const message = artifactId.length > 0 ? artifactId : 'artifactUpdate';
      this.a2aEventHub.push(eventType, message, raw);
      this.applyA2AArtifactUpdate(update);
      handled = true;
    }

    if (!handled) {
      this.a2aEventHub.push('log', 'stream_update', raw);
    }
  }

  private readArtifactId(update: A2AStreamUpdate): string {
    const artifactUpdate = update.artifactUpdate;
    const artifact = artifactUpdate ? artifactUpdate.artifact : undefined;
    if (artifact && typeof artifact.artifactId === 'string' && artifact.artifactId.length > 0) {
      return artifact.artifactId;
    }
    const artifacts = artifactUpdate ? artifactUpdate.artifacts : undefined;
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

  private readStatusMessage(statusUpdate: A2AStreamStatusUpdate): string {
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

  private applyA2AStatusUpdate(statusUpdate: A2AStreamStatusUpdate): void {
    const rawState = typeof statusUpdate.state === 'string' ? statusUpdate.state.trim() : '';
    if (rawState.length === 0) {
      return;
    }
    const state = rawState.toLowerCase();
    const previous = this.a2aSnapshot.status;
    const messageText = this.readStatusMessage(statusUpdate);

    if (state === 'working') {
      this.a2aSnapshot.status = 'running';
      this.a2aSnapshot.lastError = '';
      if (previous !== this.a2aSnapshot.status) {
        void this.handleA2AStatusChanged(this.a2aSnapshot.status, messageText);
      }
      return;
    }

    if (state === 'input-required') {
      this.a2aSnapshot.status = 'waiting_user';
      this.a2aSnapshot.lastError = '';
      if (messageText.length > 0) {
        this.a2aSnapshot.lastDescription = messageText;
      }
      const action = this.readInteractionAction(statusUpdate, messageText);
      if (action) {
        this.a2aSnapshot.lastActionName = action.name;
        this.a2aSnapshot.lastActionJson = JSON.stringify(action);
      }
      void this.handleA2AStatusChanged(this.a2aSnapshot.status, messageText);
      return;
    }

    if (state === 'completed') {
      this.a2aSnapshot.status = 'finished';
      this.a2aSnapshot.lastError = '';
      if (previous !== this.a2aSnapshot.status) {
        void this.handleA2AStatusChanged(this.a2aSnapshot.status, messageText);
      }
      return;
    }

    if (state === 'failed') {
      this.a2aSnapshot.status = 'failed';
      const reason = messageText.length > 0 ? messageText : rawState;
      this.a2aSnapshot.lastError = reason;
      if (previous !== this.a2aSnapshot.status) {
        void this.handleA2AStatusChanged(this.a2aSnapshot.status, reason);
      }
    }
  }

  private readInteractionAction(statusUpdate: A2AStreamStatusUpdate, fallbackContent: string): AgentAction | null {
    const message = statusUpdate.message;
    const metadata = message ? message.metadata : undefined;
    const metaAction = metadata ? metadata.action : undefined;
    const rawName = metaAction && typeof metaAction.name === 'string' ? metaAction.name.trim() : '';
    if (rawName.length === 0) {
      return null;
    }
    const name = rawName.toLowerCase();
    if (name !== 'call_user' && name !== 'interact' && name !== 'take_over') {
      return null;
    }
    const params: ActionParameters = {};
    const rawContent = metaAction?.parameters?.content;
    const content = typeof rawContent === 'string' && rawContent.trim().length > 0 ? rawContent.trim() : fallbackContent;
    if (content.trim().length > 0) {
      params.content = content.trim();
    }
    const rawOptions = metaAction?.parameters?.options;
    const metaOptions = metadata?.options;
    const options = Array.isArray(rawOptions) && rawOptions.length > 0 ? rawOptions : Array.isArray(metaOptions) ? metaOptions : [];
    if (options.length > 0) {
      params.options = options;
    }
    const action: AgentAction = { name, parameters: params };
    return action;
  }

  private async handleA2AStatusChanged(status: TaskStatus, messageText: string): Promise<void> {
    if (status === 'finished' || status === 'failed' || status === 'stopping' || status === 'idle') {
      this.releaseEfficiencyResourcesBestEffort();
    }

    if (status === 'finished') {
      const content = this.resolveA2AFinishContent(messageText);
      this.finalizeA2AHistoryTask('finished', content);
    } else if (status === 'failed') {
      this.finalizeA2AHistoryTask('failed', messageText);
    } else if (status === 'stopping') {
      this.finalizeA2AHistoryTask('stopped', messageText);
    }

    if (status === 'finished') {
      // Keep AgentSpace alive for transferToMain() after completion.
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
      await this.publishInteractionLiveView(messageText);
      return;
    }
    if (status === 'finished') {
      const content = this.resolveA2AFinishContent(messageText);
      await this.publishFinishedLiveView(content);
      return;
    }
    if (status === 'failed') {
      await this.publishFailedLiveView(messageText);
      return;
    }
    if (status === 'stopping') {
      await this.publishStoppedLiveView(messageText);
    }
  }

  private resolveA2AFinishContent(fallback: string): string {
    const resolved = LiveViewPresenter.resolveFinishContent(this.a2aSnapshot);
    if (resolved.trim().length > 0) {
      return resolved;
    }
    return fallback.trim();
  }

  private applyA2AArtifactUpdate(update: A2AStreamUpdate): void {
    const description = this.extractA2ADescription(update);
    if (description.length > 0) {
      this.a2aSnapshot.lastDescription = description;
    }
    const finalText = this.extractA2AFinalOutputText(update);
    if (finalText.length > 0) {
      this.applyA2AFinalOutputText(finalText);
    }
  }

  private applyA2AFinalOutputText(text: string): void {
    const content = text.trim();
    if (content.length === 0) {
      return;
    }
    const params: ActionParameters = { content };
    const action: AgentAction = { name: 'finish', parameters: params };
    this.a2aSnapshot.lastActionName = 'finish';
    this.a2aSnapshot.lastActionJson = JSON.stringify(action);
    this.finalizeA2AHistoryTask('finished', content);

    if (this.a2aSnapshot.status === 'finished' && this.liveViewEnabled) {
      void this.publishFinishedLiveView(content);
    }
  }

  private extractA2ADescription(update: A2AStreamUpdate): string {
    const artifacts = this.collectA2AArtifacts(update);
    for (let i = 0; i < artifacts.length; i += 1) {
      const artifact = artifacts[i];
      if (!artifact || artifact.artifactId !== 'thinkflow.delta') {
        continue;
      }
      const parts = artifact.parts;
      if (!Array.isArray(parts)) {
        continue;
      }
      for (let j = 0; j < parts.length; j += 1) {
        const part = parts[j];
        if (!part || part.kind !== 'data' || !part.data) {
          continue;
        }
        const delta = part.data as A2ADeltaData;
        if (delta.kind === 'agent.description' && typeof delta.description === 'string') {
          const desc = delta.description.trim();
          if (desc.length > 0) {
            return desc;
          }
        }
      }
    }
    return '';
  }

  private extractA2AFinalOutputText(update: A2AStreamUpdate): string {
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
        if (!part || part.kind !== 'data' || !part.data) {
          continue;
        }
        const output = part.data as A2AOutputData;
        const text = this.readOutputText(output);
        if (text.trim().length > 0) {
          return text.trim();
        }
      }
    }
    return '';
  }

  private readOutputText(output?: A2AOutputData): string {
    if (!output) {
      return '';
    }
    const direct = output.text;
    if (typeof direct === 'string' && direct.trim().length > 0) {
      return direct.trim();
    }
    const answer = output.answer;
    if (typeof answer === 'string' && answer.trim().length > 0) {
      return answer.trim();
    }
    const result = output.result;
    if (typeof result === 'string' && result.trim().length > 0) {
      return result.trim();
    }
    const description = output.description;
    if (typeof description === 'string' && description.trim().length > 0) {
      return description.trim();
    }

    const messageRaw = output.message;
    if (typeof messageRaw === 'string' && messageRaw.trim().length > 0) {
      return messageRaw.trim();
    }
    if (messageRaw && typeof messageRaw === 'object') {
      const messageText = this.readOutputMessageText(messageRaw as A2AOutputMessage);
      if (messageText.length > 0) {
        return messageText;
      }
    }

    const actionText = this.readOutputActionContent(output.action);
    if (actionText.length > 0) {
      return actionText;
    }

    const nested = this.readOutputText(output.output);
    if (nested.length > 0) {
      return nested;
    }
    const nestedFinal = this.readOutputText(output.final);
    if (nestedFinal.length > 0) {
      return nestedFinal;
    }
    return '';
  }

  private readOutputActionContent(action?: A2AOutputAction): string {
    if (!action) {
      return '';
    }
    const params = action.parameters;
    const content = params?.content;
    if (typeof content === 'string' && content.trim().length > 0) {
      return content.trim();
    }
    const message = params?.message;
    if (typeof message === 'string' && message.trim().length > 0) {
      return message.trim();
    }
    return '';
  }

  private readOutputMessageText(message?: A2AOutputMessage): string {
    if (!message) {
      return '';
    }
    const parts = message.parts;
    if (!Array.isArray(parts)) {
      return '';
    }
    for (let i = 0; i < parts.length; i += 1) {
      const part = parts[i];
      if (!part || part.kind !== 'text') {
        continue;
      }
      if (typeof part.text === 'string' && part.text.trim().length > 0) {
        return part.text.trim();
      }
    }
    return '';
  }

  private collectA2AArtifacts(update: A2AStreamUpdate): Array<A2AStreamArtifact> {
    const artifacts: Array<A2AStreamArtifact> = [];
    const artifactUpdate = update.artifactUpdate;
    const single = artifactUpdate ? artifactUpdate.artifact : undefined;
    if (single) {
      artifacts.push(single);
    }
    const list = artifactUpdate ? artifactUpdate.artifacts : undefined;
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

  private safeStringify(value: Object): string {
    try {
      const raw = JSON.stringify(value);
      return typeof raw === 'string' ? raw : '';
    } catch (_e) {
      return '';
    }
  }

  private async buildA2ARequest(
    text: string,
    taskId: string,
    includeApps: boolean,
    contextIdOverride: string = ''
  ): Promise<A2ARequest> {
    const ctx = this.ctx;
    if (!ctx) {
      throw new Error('context_missing');
    }
    const settings = await AgentSettings.load(ctx);
    const override = contextIdOverride.trim();
    const contextId = override.length > 0
      ? override
      : (settings.contextId.trim().length > 0 ? settings.contextId.trim() : settings.deviceId.trim());
    if (contextId.length === 0) {
      throw new Error('context_id_empty');
    }
    const screenshot = await this.requireToolKit().screenshotBase64();
    if (screenshot.length === 0) {
      throw new Error('screenshot_empty');
    }
    const textPart: A2ATextPart = { kind: 'text', text };
    const file: A2AFile = { name: 'screenshot.jpeg', mimeType: 'image/jpeg', bytes: screenshot };
    const filePart: A2AFilePart = { kind: 'file', file };
    const parts: Array<A2AMessagePart> = [textPart, filePart];
    const message: A2AMessage = {
      messageId: `m_${Date.now()}`,
      role: 'user',
      contextId,
      parts,
    };
    if (taskId.trim().length > 0) {
      message.taskId = taskId.trim();
    }
    const config: A2AConfiguration = { blocking: true };
    const request: A2ARequest = { message, configuration: config };
    if (includeApps) {
      const apps = await this.safeListAppNames(ctx);
      const metadata: A2AMetadata = { installed_apps: apps };
      request.metadata = metadata;
    }
    return request;
  }

  private appendMcpLog(message: string): void {
    const now = new Date();
    const hh = String(now.getHours()).padStart(2, '0');
    const mm = String(now.getMinutes()).padStart(2, '0');
    const ss = String(now.getSeconds()).padStart(2, '0');
    const line = `${hh}:${mm}:${ss} ${message}`;
    const next = this.mcpLogLines.slice();
    next.push(line);
    const limit = 120;
    while (next.length > limit) {
      next.shift();
    }
    this.mcpLogLines = next;
  }

  private buildDeviceToolsConfig(settings: AgentSettingsData): DeviceToolsConfig {
    const contextId = settings.contextId.trim().length > 0 ? settings.contextId.trim() : settings.deviceId.trim();
    return {
      wsUrl: settings.mcpWsUrl,
      deviceId: settings.deviceId,
      sessionId: contextId,
      waitAfterActionMs: settings.waitMs,
    };
  }

  private async ensureMcpConnected(settings: AgentSettingsData, sessionIdOverride: string = ''): Promise<void> {
    const config = this.buildDeviceToolsConfig(settings);
    const override = sessionIdOverride.trim();
    if (override.length > 0) {
      config.sessionId = override;
    }
    const targetUrl = config.wsUrl.trim();
    if (targetUrl.length === 0) {
      return;
    }
    const sessionId = config.sessionId.trim();
    if (sessionId.length === 0) {
      this.appendMcpLog('auto_connect_error: session_id_empty');
      return;
    }
    try {
      const ctx = this.ctx;
      if (ctx) {
        const toolKit = this.requireToolKit();
        const targets = await this.safeListAppTargets(ctx);
        toolKit.setAppTargets(targets);
        this.appTargetMap = this.buildAppTargetMap(targets);
      }
      const endpoint = this.requireDeviceToolsEndpoint();
      const status = endpoint.getStatus();
      const sessionMatch = this.mcpSessionId === sessionId;
      if ((status.state === 'connected' || status.state === 'connecting') && status.url === targetUrl && sessionMatch) {
        return;
      }
      if (status.state === 'connected' || status.state === 'connecting') {
        await endpoint.disconnect();
      }
      await endpoint.connect(config);
      this.mcpSessionId = sessionId;
    } catch (e) {
      this.appendMcpLog(`auto_connect_error: ${String(e)}`);
    }
  }

  private async loadDeviceToolsConfig(): Promise<DeviceToolsConfig> {
    const ctx = this.ctx;
    const fallbackSettings = AgentSettings.defaults();
    if (!ctx) {
      return this.buildDeviceToolsConfig(fallbackSettings);
    }
    try {
      const settings = await AgentSettings.load(ctx);
      return this.buildDeviceToolsConfig(settings);
    } catch (_e) {
      return this.buildDeviceToolsConfig(fallbackSettings);
    }
  }

  private async resolveToolWaitMs(): Promise<number> {
    const ctx = this.ctx;
    if (!ctx) {
      return AgentSettings.defaults().waitMs;
    }
    try {
      const settings = await AgentSettings.load(ctx);
      return settings.waitMs;
    } catch (_e) {
      return AgentSettings.defaults().waitMs;
    }
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
    const space = new AgentSpace(invoker, undefined, undefined, this.ctx ?? undefined);
    this.agentSpace = space;
    this.toolKit = new ToolKit(space);
  }

  private async destroyAgentSpaceInternal(): Promise<void> {
    this.pendingAgentSpaceDestroy = false;
    const space = this.agentSpace;
    const endpoint = this.deviceToolsEndpoint;
    this.agentSpace = null;
    this.toolKit = null;
    this.deviceToolsEndpoint = null;
    this.deviceToolsToolKit = null;
    this.lastScreenshotBase64 = '';
    this.lastLongScreenshotBase64 = '';
    this.mcpSessionId = '';
    if (endpoint) {
      void endpoint.disconnect();
    }
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

class AgentControllerDeviceToolsObserver implements DeviceToolsEndpointObserver {
  private readonly owner: AgentController;

  constructor(owner: AgentController) {
    this.owner = owner;
  }

  onToolCallStarted(call: McpToolCall): void {
    this.owner.handleDeviceToolCallStarted(call);
  }

  onToolCallCompleted(call: McpToolCall, result: ToolCallResult, elapsedMs: number): void {
    this.owner.handleDeviceToolCallCompleted(call, result, elapsedMs);
  }
}
