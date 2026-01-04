import type { AgentAction, ActionParameters, ActionResult } from '../agentspace/AgentSpace';
import {
  CloudAgentClient,
  CloudControl,
  CloudDeviceState,
  CloudInvokeError,
  CloudInvokePayload,
  CloudInvokeResponse,
  CloudInvokeOutput
} from './CloudAgentClient';
import { DeviceExecutor } from './DeviceExecutor';
import type { TaskEvent, TaskEventBatch, TaskEventType, TaskSnapshot, TaskStartRequest, UserReply } from './types';
import { hilog } from '@kit.PerformanceAnalysisKit';

const DEFAULT_WAIT_AFTER_ACTION_MS: number = 500;
const MAX_RETRY_COUNT: number = 3;
const HILOG_DOMAIN: number = 0x0000;
const HILOG_TAG: string = 'TaskManager';

interface NeedUserPayload {
  description: string;
  action: AgentAction;
}

export interface TaskManagerListener {
  onStatusChanged(snapshot: TaskSnapshot): void;
  onAction(action: AgentAction, description: string): void;
}

export class TaskManager {
  private snapshot: TaskSnapshot = {
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
  private eventSequence: number = 0;
  private events: Array<TaskEvent> = [];
  private runToken: number = 0;
  private stopRequested: boolean = false;
  private cloudClient: CloudAgentClient | null = null;
  private deviceExecutor: DeviceExecutor | null = null;
  private waitAfterActionMs: number = DEFAULT_WAIT_AFTER_ACTION_MS;
  private taskText: string = '';
  private installedApps: Array<string> = [];
  private installedAppsSent: boolean = false;
  private pendingActionResult: ActionResult | null = null;
  private pendingUserNote: string = '';
  private pendingControlOp: string = '';
  private listener: TaskManagerListener | null = null;

  startTask(request: TaskStartRequest, cloudClient: CloudAgentClient, deviceExecutor: DeviceExecutor): void {
    const taskText = request.taskText.trim();
    if (taskText.length === 0) {
      this.failTask('task_text_empty');
      return;
    }
    this.cloudClient = cloudClient;
    this.deviceExecutor = deviceExecutor;
    this.waitAfterActionMs = this.normalizeWaitMs(request.waitMs);
    this.taskText = taskText;
    const installed = Array.isArray(request.installedApps) ? request.installedApps : [];
    this.installedApps = installed.slice();
    this.installedAppsSent = false;
    this.pendingActionResult = null;
    this.pendingUserNote = '';
    this.pendingControlOp = '';
    this.stopRequested = false;
    this.runToken += 1;
    const token = this.runToken;
    this.eventSequence = 0;
    this.events = [];
    this.snapshot = {
      sessionId: request.sessionId.trim(),
      status: 'running',
      stepIndex: 0,
      lastDescription: '',
      lastActionName: '',
      lastActionJson: '',
      lastResultStatus: '',
      lastError: '',
      installedAppsSent: false,
    };
    this.pushEvent('status', 'running');
    this.notifyStatus();
    void this.runLoop(token);
  }

  submitUserReply(reply: UserReply): void {
    if (this.snapshot.status !== 'waiting_user') {
      this.pushEvent('error', 'not_waiting_user');
      return;
    }
    const content = reply.content.trim();
    if (content.length === 0) {
      this.pushEvent('error', 'empty_user_reply');
      return;
    }
    if (reply.kind === 'task_over') {
      this.pendingUserNote = content;
      this.pendingControlOp = 'rollback';
    }
    const result: ActionResult = { status: 'SKIPPED', output: content };
    this.pendingActionResult = result;
    this.snapshot.lastResultStatus = result.status;
    this.snapshot.lastError = '';
    this.pushEvent('result', result.status, JSON.stringify(result));
    this.snapshot.status = 'running';
    this.pushEvent('status', 'running');
    this.notifyStatus();
    this.stopRequested = false;
    this.runToken += 1;
    const token = this.runToken;
    void this.runLoop(token);
  }

  requestSupplement(content: string): void {
    const trimmed = content.trim();
    if (trimmed.length === 0) {
      this.pushEvent('error', 'supplement_empty');
      return;
    }
    if (this.snapshot.status === 'idle') {
      this.pushEvent('error', 'supplement_idle');
      return;
    }
    this.stopRequested = true;
    this.snapshot.status = 'waiting_user';
    this.snapshot.lastDescription = 'supplement';
    this.snapshot.lastActionName = 'call_user';
    const params: ActionParameters = { content: trimmed };
    const action: AgentAction = { name: 'call_user', parameters: params };
    this.snapshot.lastActionJson = JSON.stringify(action);
    this.pushEvent('need_user', 'supplement', this.snapshot.lastActionJson);
    this.pushEvent('status', 'waiting_user');
    this.notifyStatus();
  }

  submitUserNote(content: string): void {
    const trimmed = content.trim();
    if (trimmed.length === 0) {
      this.pushEvent('error', 'user_note_empty');
      return;
    }
    if (this.snapshot.status === 'idle') {
      this.pushEvent('error', 'user_note_idle');
      return;
    }
    this.pendingUserNote = trimmed;
    this.pendingControlOp = '';
    const result: ActionResult = { status: 'SKIPPED' };
    this.pendingActionResult = result;
    this.snapshot.lastResultStatus = result.status;
    this.snapshot.lastError = '';
    this.pushEvent('result', result.status, JSON.stringify(result));
    this.snapshot.status = 'running';
    this.pushEvent('status', 'running');
    this.notifyStatus();
    this.stopRequested = false;
    this.runToken += 1;
    const token = this.runToken;
    void this.runLoop(token);
  }

  debugCallUser(content: string): void {
    const trimmed = content.trim();
    if (trimmed.length === 0) {
      this.pushEvent('error', 'debug_call_user_empty');
      return;
    }
    this.stopRequested = true;
    this.snapshot.status = 'waiting_user';
    this.snapshot.lastDescription = 'call_user';
    this.snapshot.lastActionName = 'call_user';
    const params: ActionParameters = { content: trimmed };
    const action: AgentAction = { name: 'call_user', parameters: params };
    this.snapshot.lastActionJson = JSON.stringify(action);
    this.pushEvent('need_user', 'call_user', this.snapshot.lastActionJson);
    this.pushEvent('status', 'waiting_user');
    this.notifyStatus();
  }

  debugTakeOver(content: string): void {
    const trimmed = content.trim();
    if (trimmed.length === 0) {
      this.pushEvent('error', 'debug_take_over_empty');
      return;
    }
    this.stopRequested = true;
    this.snapshot.status = 'waiting_user';
    this.snapshot.lastDescription = 'take_over';
    this.snapshot.lastActionName = 'take_over';
    const params: ActionParameters = { content: trimmed };
    const action: AgentAction = { name: 'take_over', parameters: params };
    this.snapshot.lastActionJson = JSON.stringify(action);
    this.pushEvent('need_user', 'take_over', this.snapshot.lastActionJson);
    this.pushEvent('status', 'waiting_user');
    this.notifyStatus();
  }

  requestTaskOver(content: string): void {
    const trimmed = content.trim();
    if (trimmed.length === 0) {
      this.pushEvent('error', 'task_over_empty');
      return;
    }
    if (this.snapshot.status === 'idle') {
      this.pushEvent('error', 'task_over_idle');
      return;
    }
    this.stopRequested = true;
    this.snapshot.status = 'waiting_user';
    this.snapshot.lastDescription = 'task_over';
    this.snapshot.lastActionName = 'task_over';
    const params: ActionParameters = { content: trimmed };
    const action: AgentAction = { name: 'task_over', parameters: params };
    this.snapshot.lastActionJson = JSON.stringify(action);
    this.pushEvent('need_user', 'task_over', this.snapshot.lastActionJson);
    this.pushEvent('status', 'waiting_user');
    this.notifyStatus();
  }

  debugInteract(content: string, options: Array<string>): void {
    const trimmed = content.trim();
    if (trimmed.length === 0) {
      this.pushEvent('error', 'debug_interact_empty');
      return;
    }
    const normalizedOptions = this.normalizeOptionList(options);
    if (normalizedOptions.length < 2) {
      this.pushEvent('error', 'debug_interact_options_short');
      return;
    }
    this.stopRequested = true;
    this.snapshot.status = 'waiting_user';
    this.snapshot.lastDescription = 'interact';
    this.snapshot.lastActionName = 'interact';
    const params: ActionParameters = { content: trimmed, options: normalizedOptions };
    const action: AgentAction = { name: 'interact', parameters: params };
    this.snapshot.lastActionJson = JSON.stringify(action);
    this.pushEvent('need_user', 'interact', this.snapshot.lastActionJson);
    this.pushEvent('status', 'waiting_user');
    this.notifyStatus();
  }

  stopTask(reason: string): void {
    if (this.snapshot.status === 'idle') {
      return;
    }
    this.stopRequested = true;
    this.snapshot.status = 'stopping';
    this.snapshot.lastError = reason;
    this.pushEvent('status', `stopping:${reason}`);
    this.notifyStatus();
  }

  setListener(listener: TaskManagerListener | null): void {
    this.listener = listener;
  }

  getSnapshot(): TaskSnapshot {
    const copy: TaskSnapshot = {
      sessionId: this.snapshot.sessionId,
      status: this.snapshot.status,
      stepIndex: this.snapshot.stepIndex,
      lastDescription: this.snapshot.lastDescription,
      lastActionName: this.snapshot.lastActionName,
      lastActionJson: this.snapshot.lastActionJson,
      lastResultStatus: this.snapshot.lastResultStatus,
      lastError: this.snapshot.lastError,
      installedAppsSent: this.snapshot.installedAppsSent,
    };
    return copy;
  }

  pullEvents(lastSequence: number): TaskEventBatch | null {
    if (this.events.length === 0 || this.eventSequence === lastSequence) {
      return null;
    }
    const batch: TaskEventBatch = {
      sequence: this.eventSequence,
      events: this.events,
    };
    this.events = [];
    return batch;
  }

  private async runLoop(token: number): Promise<void> {
    const cloud = this.cloudClient;
    const device = this.deviceExecutor;
    if (!cloud || !device) {
      this.failTask('dependencies_missing');
      return;
    }
    let sessionId = this.snapshot.sessionId;
    let pendingAction: AgentAction | null = null;
    let pendingResult: ActionResult | null = this.pendingActionResult;
    this.pendingActionResult = null;
    let pendingDescription: string = '';
    hilog.info(HILOG_DOMAIN, HILOG_TAG, 'runLoop start token=%{public}d', token);
    while (!this.stopRequested && token === this.runToken) {
      if (this.shouldAbortLoop()) {
        break;
      }
      if (!pendingAction) {
        try {
          if (this.shouldAbortLoop()) {
            break;
          }
          hilog.info(HILOG_DOMAIN, HILOG_TAG, 'token=%{public}d step=%{public}d screenshot(initial) begin', token, this.snapshot.stepIndex);
          const screenshot = await device.screenshotBase64();
          if (this.shouldAbortLoop()) {
            break;
          }
          hilog.info(HILOG_DOMAIN, HILOG_TAG, 'token=%{public}d step=%{public}d screenshot(initial) done len=%{public}d', token, this.snapshot.stepIndex, screenshot.length);
          const payload = pendingResult
            ? this.buildFollowupPayload(screenshot, pendingResult)
            : this.buildInitialPayload(screenshot);
          hilog.info(HILOG_DOMAIN, HILOG_TAG, 'token=%{public}d step=%{public}d invoke(initial) begin', token, this.snapshot.stepIndex);
          const response = await this.invokeWithRetry(cloud, sessionId, payload, token);
          if (this.shouldAbortLoop()) {
            break;
          }
          hilog.info(HILOG_DOMAIN, HILOG_TAG, 'token=%{public}d step=%{public}d invoke(initial) done', token, this.snapshot.stepIndex);
          pendingResult = null;
          sessionId = this.applySessionId(response, sessionId);
          const output = response.output;
          const action = this.extractAction(cloud, output);
          if (!action) {
            this.failTask('missing_action');
            break;
          }
          pendingDescription = output?.description ?? '';
          const outcome = this.handleIncomingAction(action, pendingDescription);
          if (outcome !== 'continue') {
            break;
          }
          pendingAction = action;
        } catch (e) {
          const err = e instanceof Error ? e : new Error(String(e));
          if (err.message === 'stopped' && this.shouldAbortLoop()) {
            break;
          }
          this.failTask(`invoke_failed:${err.message}`);
          break;
        }
        continue;
      }

      try {
        if (this.shouldAbortLoop()) {
          break;
        }
        hilog.info(HILOG_DOMAIN, HILOG_TAG, 'token=%{public}d step=%{public}d executeAction begin name=%{public}s', token, this.snapshot.stepIndex, pendingAction.name);
        const result = await this.executeAction(device, pendingAction);
        if (this.shouldAbortLoop()) {
          break;
        }
        hilog.info(HILOG_DOMAIN, HILOG_TAG, 'token=%{public}d step=%{public}d executeAction done status=%{public}s', token, this.snapshot.stepIndex, result.status);
        this.recordActionResult(result);
        hilog.info(HILOG_DOMAIN, HILOG_TAG, 'token=%{public}d step=%{public}d waitAfterAction begin ms=%{public}d', token, this.snapshot.stepIndex, this.waitAfterActionMs);
        await this.waitAfterAction(token);
        if (this.shouldAbortLoop()) {
          break;
        }
        hilog.info(HILOG_DOMAIN, HILOG_TAG, 'token=%{public}d step=%{public}d waitAfterAction done', token, this.snapshot.stepIndex);
        hilog.info(HILOG_DOMAIN, HILOG_TAG, 'token=%{public}d step=%{public}d screenshot(after) begin', token, this.snapshot.stepIndex);
        const screenshot = await device.screenshotBase64();
        if (this.shouldAbortLoop()) {
          break;
        }
        hilog.info(HILOG_DOMAIN, HILOG_TAG, 'token=%{public}d step=%{public}d screenshot(after) done len=%{public}d', token, this.snapshot.stepIndex, screenshot.length);
        const payload = this.buildFollowupPayload(screenshot, result);
        hilog.info(HILOG_DOMAIN, HILOG_TAG, 'token=%{public}d step=%{public}d invoke(after) begin', token, this.snapshot.stepIndex);
        const response = await this.invokeWithRetry(cloud, sessionId, payload, token);
        if (this.shouldAbortLoop()) {
          break;
        }
        hilog.info(HILOG_DOMAIN, HILOG_TAG, 'token=%{public}d step=%{public}d invoke(after) done', token, this.snapshot.stepIndex);
        sessionId = this.applySessionId(response, sessionId);
        const output = response.output;
        const action = this.extractAction(cloud, output);
        if (!action) {
          this.failTask('missing_action');
          break;
        }
        pendingDescription = output?.description ?? '';
        const outcome = this.handleIncomingAction(action, pendingDescription);
        if (outcome !== 'continue') {
          break;
        }
        pendingAction = action;
      } catch (e) {
        const err = e instanceof Error ? e : new Error(String(e));
        if (err.message === 'stopped' && this.shouldAbortLoop()) {
          break;
        }
        this.failTask(`action_failed:${err.message}`);
        break;
      }
    }

    if (token !== this.runToken) {
      return;
    }
    if (this.stopRequested && (this.snapshot.status === 'stopping' || this.snapshot.status === 'running')) {
      this.snapshot.status = 'idle';
      this.pushEvent('status', 'idle');
      this.notifyStatus();
    }
    hilog.info(HILOG_DOMAIN, HILOG_TAG, 'runLoop end token=%{public}d status=%{public}s', token, this.snapshot.status);
  }

  private buildInitialPayload(screenshot: string): CloudInvokePayload {
    const state = this.buildStatePayload(screenshot);
    const payload: CloudInvokePayload = {
      task: this.taskText,
      state,
    };
    const control = this.consumeControl();
    if (control) {
      payload.control = control;
    }
    return payload;
  }

  private buildFollowupPayload(screenshot: string, result: ActionResult): CloudInvokePayload {
    const state = this.buildStatePayload(screenshot);
    const payload: CloudInvokePayload = {
      state,
      action_result: result,
    };
    const control = this.consumeControl();
    if (control) {
      payload.control = control;
    }
    return payload;
  }

  private buildStatePayload(screenshot: string): CloudDeviceState {
    const state: CloudDeviceState = { screenshot };
    const note = this.consumeUserNote();
    if (note.length > 0) {
      state.user_note = note;
    }
    if (!this.installedAppsSent && this.installedApps.length > 0) {
      state.installed_apps = this.installedApps;
      this.installedAppsSent = true;
      this.snapshot.installedAppsSent = true;
    }
    return state;
  }

  private applySessionId(response: CloudInvokeResponse, fallback: string): string {
    const outputId = response.output?.session_id ?? '';
    const envelopeId = response.session_id ?? '';
    const next = envelopeId.length > 0 ? envelopeId : outputId.length > 0 ? outputId : fallback;
    if (next !== this.snapshot.sessionId) {
      this.snapshot.sessionId = next;
    }
    return next;
  }

  private extractAction(cloud: CloudAgentClient, output?: CloudInvokeOutput): AgentAction | null {
    if (!output || !output.action) {
      return null;
    }
    return cloud.normalizeAction(output.action);
  }

  private handleIncomingAction(action: AgentAction, description: string): 'continue' | 'finish' | 'wait_user' {
    const name = action.name.trim();
    if (name.length === 0) {
      this.failTask('empty_action');
      return 'finish';
    }
    this.snapshot.stepIndex += 1;
    this.snapshot.lastDescription = description;
    this.snapshot.lastActionName = name;
    this.snapshot.lastActionJson = JSON.stringify(action);
    this.pushEvent('action', name, this.snapshot.lastActionJson);
    this.notifyAction(action, description);

    const lower = name.toLowerCase();
    if (lower === 'finish') {
      this.snapshot.status = 'finished';
      this.pushEvent('status', 'finished');
      this.notifyStatus();
      return 'finish';
    }
    if (lower === 'call_user' || lower === 'interact' || lower === 'task_over' || lower === 'take_over') {
      this.snapshot.status = 'waiting_user';
      const payload: NeedUserPayload = { description, action };
      this.pushEvent('need_user', lower, JSON.stringify(payload));
      this.pushEvent('status', 'waiting_user');
      this.notifyStatus();
      return 'wait_user';
    }
    return 'continue';
  }

  private async executeAction(device: DeviceExecutor, action: AgentAction): Promise<ActionResult> {
    const lower = action.name.trim().toLowerCase();
    if (lower === 'take_note') {
      const result: ActionResult = { status: 'SUCCESS' };
      return result;
    }
    return await device.executeAction(action);
  }

  private recordActionResult(result: ActionResult): void {
    this.snapshot.lastResultStatus = result.status;
    this.snapshot.lastError = result.error ?? '';
    this.pushEvent('result', result.status, JSON.stringify(result));
  }

  private async invokeWithRetry(
    cloud: CloudAgentClient,
    sessionId: string,
    payload: CloudInvokePayload,
    token: number
  ): Promise<CloudInvokeResponse> {
    let attempt = 0;
    while (true) {
      if (this.stopRequested || token !== this.runToken) {
        throw new Error('stopped');
      }
      try {
        return await cloud.invoke(sessionId, payload);
      } catch (e) {
        const err = e instanceof Error ? e : new Error(String(e));
        const status = this.readStatusCode(err);
        if (status === 409 || status === 408 || status === 502) {
          attempt += 1;
          if (attempt > MAX_RETRY_COUNT) {
            throw err;
          }
          const delay = this.retryDelay(attempt);
          this.pushEvent('log', `retry:${status} in ${delay}ms`);
          await sleep(delay);
          continue;
        }
        throw err;
      }
    }
  }

  private readStatusCode(error: Error): number {
    if (error instanceof CloudInvokeError) {
      return error.statusCode;
    }
    return -1;
  }

  private retryDelay(attempt: number): number {
    const base = 400;
    const delay = base * attempt;
    return Math.min(2000, delay);
  }

  private async waitAfterAction(token: number): Promise<void> {
    if (this.waitAfterActionMs <= 0) {
      return;
    }
    if (this.stopRequested || token !== this.runToken) {
      return;
    }
    await sleep(this.waitAfterActionMs);
  }

  private normalizeOptionList(options: Array<string>): Array<string> {
    const result: Array<string> = [];
    for (let i = 0; i < options.length; i++) {
      const value = options[i];
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

  private shouldAbortLoop(): boolean {
    return this.stopRequested &&
      (this.snapshot.status === 'waiting_user' || this.snapshot.status === 'stopping');
  }

  private consumeUserNote(): string {
    const note = this.pendingUserNote.trim();
    this.pendingUserNote = '';
    return note;
  }

  private consumeControl(): CloudControl | null {
    const op = this.pendingControlOp.trim();
    this.pendingControlOp = '';
    if (op.length === 0) {
      return null;
    }
    return { op };
  }

  private normalizeWaitMs(value: number): number {
    if (!Number.isFinite(value)) {
      return DEFAULT_WAIT_AFTER_ACTION_MS;
    }
    return Math.max(0, Math.floor(value));
  }

  private failTask(reason: string): void {
    this.stopRequested = true;
    this.snapshot.status = 'failed';
    this.snapshot.lastError = reason;
    this.pushEvent('error', reason);
    this.pushEvent('status', 'failed');
    this.notifyStatus();
  }

  private notifyStatus(): void {
    if (!this.listener) {
      return;
    }
    this.listener.onStatusChanged(this.getSnapshot());
  }

  private notifyAction(action: AgentAction, description: string): void {
    if (!this.listener) {
      return;
    }
    this.listener.onAction(action, description);
  }

  private pushEvent(type: TaskEventType, message: string, payloadJson?: string): void {
    this.eventSequence += 1;
    const event: TaskEvent = {
      seq: this.eventSequence,
      type,
      message,
      payloadJson: payloadJson ?? '',
    };
    this.events.push(event);
  }
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
