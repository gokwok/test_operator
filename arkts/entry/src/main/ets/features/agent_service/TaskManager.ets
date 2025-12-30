import type { AgentAction, ActionResult } from '../agentspace/AgentSpace';
import {
  CloudAgentClient,
  CloudDeviceState,
  CloudInvokeError,
  CloudInvokePayload,
  CloudInvokeResponse,
  CloudInvokeOutput
} from './CloudAgentClient';
import { DeviceExecutor } from './DeviceExecutor';
import type { TaskEvent, TaskEventBatch, TaskEventType, TaskSnapshot, TaskStartRequest, UserReply } from './types';

const DEFAULT_WAIT_AFTER_ACTION_MS: number = 500;
const MAX_RETRY_COUNT: number = 3;

interface NeedUserPayload {
  description: string;
  action: AgentAction;
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
    const result: ActionResult = { status: 'SKIPPED', output: content };
    this.pendingActionResult = result;
    this.snapshot.lastResultStatus = result.status;
    this.snapshot.lastError = '';
    this.pushEvent('result', result.status, JSON.stringify(result));
    this.snapshot.status = 'running';
    this.pushEvent('status', 'running');
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
    this.snapshot.lastActionJson = JSON.stringify({
      name: 'call_user',
      parameters: { content: trimmed }
    });
    this.pushEvent('need_user', 'call_user', this.snapshot.lastActionJson);
    this.pushEvent('status', 'waiting_user');
  }

  stopTask(reason: string): void {
    if (this.snapshot.status === 'idle') {
      return;
    }
    this.stopRequested = true;
    this.snapshot.status = 'stopping';
    this.snapshot.lastError = reason;
    this.pushEvent('status', `stopping:${reason}`);
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
    while (!this.stopRequested && token === this.runToken) {
      if (!pendingAction) {
        try {
          const screenshot = await device.screenshotBase64();
          const payload = pendingResult
            ? this.buildFollowupPayload(screenshot, pendingResult)
            : this.buildInitialPayload(screenshot);
          const response = await this.invokeWithRetry(cloud, sessionId, payload, token);
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
          this.failTask(`invoke_failed:${err.message}`);
          break;
        }
        continue;
      }

      try {
        const result = await this.executeAction(device, pendingAction);
        this.recordActionResult(result);
        await this.waitAfterAction(token);
        const screenshot = await device.screenshotBase64();
        const payload = this.buildFollowupPayload(screenshot, result);
        const response = await this.invokeWithRetry(cloud, sessionId, payload, token);
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
    }
  }

  private buildInitialPayload(screenshot: string): CloudInvokePayload {
    const state = this.buildStatePayload(screenshot);
    const payload: CloudInvokePayload = {
      task: this.taskText,
      state,
    };
    return payload;
  }

  private buildFollowupPayload(screenshot: string, result: ActionResult): CloudInvokePayload {
    const state = this.buildStatePayload(screenshot);
    const payload: CloudInvokePayload = {
      state,
      action_result: result,
    };
    return payload;
  }

  private buildStatePayload(screenshot: string): CloudDeviceState {
    const state: CloudDeviceState = { screenshot };
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

    const lower = name.toLowerCase();
    if (lower === 'finish') {
      this.snapshot.status = 'finished';
      this.pushEvent('status', 'finished');
      return 'finish';
    }
    if (lower === 'call_user' || lower === 'interact' || lower === 'task_over') {
      this.snapshot.status = 'waiting_user';
      const payload: NeedUserPayload = { description, action };
      this.pushEvent('need_user', lower, JSON.stringify(payload));
      this.pushEvent('status', 'waiting_user');
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
