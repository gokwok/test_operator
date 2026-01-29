import type { AgentAction, ActionParameters, ActionResult } from '../agentspace/AgentSpace';
import { ToolKit } from '../toolkit/ToolKit';

export type ToolCallStatus = 'SUCCESS' | 'FAILED';

export interface ToolCallResult {
  status: ToolCallStatus;
  error: string;
  screenshot_b64: string;
}

interface ToolCallArgs {
  point?: Array<number>;
  start?: Array<number>;
  end?: Array<number>;
  duration_ms?: number;
  seconds?: number;
  content?: string;
  app?: string;
}

const DEFAULT_LONG_CLICK_MS: number = 1500;
const DEFAULT_SWIPE_MS: number = 300;
const DEFAULT_WAIT_SECONDS: number = 3;

export class ToolCallDispatcher {
  private toolKit: ToolKit;
  private waitAfterActionMs: number;

  constructor(toolKit: ToolKit, waitAfterActionMs?: number) {
    this.toolKit = toolKit;
    this.waitAfterActionMs = this.normalizeWaitMs(waitAfterActionMs);
  }

  async execute(name: string, argsJson: string): Promise<ToolCallResult> {
    const toolName = name.trim().toLowerCase();
    if (toolName.length === 0) {
      return { status: 'FAILED', error: 'empty_tool_name', screenshot_b64: '' };
    }
    const args = this.parseArgs(argsJson);
    if (!args) {
      const snapshot = await this.safeScreenshot();
      return { status: 'FAILED', error: 'invalid_arguments_json', screenshot_b64: snapshot };
    }
    if (toolName === 'screenshot') {
      const snapshot = await this.safeScreenshot();
      if (snapshot.length === 0) {
        return { status: 'FAILED', error: 'screenshot_failed', screenshot_b64: snapshot };
      }
      return { status: 'SUCCESS', error: '', screenshot_b64: snapshot };
    }

    let status: ToolCallStatus = 'SUCCESS';
    let error: string = '';
    let actionExecuted = false;
    try {
      const action = this.buildAction(toolName, args);
      if (!action) {
        status = 'FAILED';
        error = 'invalid_arguments';
      } else {
        actionExecuted = true;
        const result = await this.toolKit.executeAction(action);
        if (result.status !== 'SUCCESS') {
          status = 'FAILED';
          error = this.formatActionError(result);
        }
      }
    } catch (e) {
      status = 'FAILED';
      error = String(e);
    }

    if (actionExecuted && this.shouldWaitAfterAction(toolName)) {
      await sleep(this.waitAfterActionMs);
    }

    const snapshot = await this.safeScreenshot();
    if (snapshot.length === 0 && status === 'SUCCESS') {
      status = 'FAILED';
      error = error.length > 0 ? error : 'screenshot_failed';
    }
    return { status, error, screenshot_b64: snapshot };
  }

  private buildAction(name: string, args: ToolCallArgs): AgentAction | null {
    if (name === 'click' || name === 'double_click') {
      const point = this.readPoint(args.point);
      if (!point) {
        return null;
      }
      const params: ActionParameters = { point };
      return { name, parameters: params };
    }
    if (name === 'long_click') {
      const point = this.readPoint(args.point);
      if (!point) {
        return null;
      }
      const duration = this.readNumber(args.duration_ms, DEFAULT_LONG_CLICK_MS);
      const params: ActionParameters = { point, duration_ms: duration };
      return { name, parameters: params };
    }
    if (name === 'swipe') {
      const start = this.readPoint(args.start);
      const end = this.readPoint(args.end);
      if (!start || !end) {
        return null;
      }
      const duration = this.readNumber(args.duration_ms, DEFAULT_SWIPE_MS);
      const params: ActionParameters = {
        start_point: start,
        end_point: end,
        duration_ms: duration,
      };
      return { name, parameters: params };
    }
    if (name === 'type') {
      const content = this.readText(args.content);
      if (content.length === 0) {
        return null;
      }
      const params: ActionParameters = { content };
      const point = this.readPoint(args.point);
      if (point) {
        params.point = point;
      }
      return { name, parameters: params };
    }
    if (name === 'wait') {
      const seconds = this.readNumber(args.seconds, DEFAULT_WAIT_SECONDS);
      const params: ActionParameters = { seconds };
      return { name, parameters: params };
    }
    if (name === 'launch_app') {
      const app = this.readText(args.app);
      if (app.length === 0) {
        return null;
      }
      const params: ActionParameters = { app };
      return { name, parameters: params };
    }
    if (name === 'press_back' || name === 'press_enter') {
      return { name };
    }
    return null;
  }

  private parseArgs(argsJson: string): ToolCallArgs | null {
    const trimmed = argsJson.trim();
    if (trimmed.length === 0) {
      return {};
    }
    try {
      const parsed = JSON.parse(trimmed);
      if (!parsed || typeof parsed !== 'object') {
        return null;
      }
      if (Array.isArray(parsed)) {
        return null;
      }
      return parsed as ToolCallArgs;
    } catch (_e) {
      return null;
    }
  }

  private readPoint(raw?: Array<number>): Array<number> | null {
    if (!raw || raw.length < 2) {
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

  private readText(raw?: Object): string {
    if (typeof raw === 'string') {
      return raw.trim();
    }
    return '';
  }

  private readNumber(raw: number | undefined, fallback: number): number {
    if (typeof raw === 'number' && Number.isFinite(raw)) {
      return raw;
    }
    return fallback;
  }

  private normalizeWaitMs(raw?: number): number {
    if (typeof raw === 'number' && Number.isFinite(raw)) {
      return Math.max(0, Math.floor(raw));
    }
    return 300;
  }

  private shouldWaitAfterAction(toolName: string): boolean {
    if (this.waitAfterActionMs <= 0) {
      return false;
    }
    return toolName !== 'screenshot' && toolName !== 'wait';
  }

  private formatActionError(result: ActionResult): string {
    if (result.error && result.error.length > 0) {
      return result.error;
    }
    if (result.status !== 'SUCCESS') {
      return result.status;
    }
    return '';
  }

  private async safeScreenshot(): Promise<string> {
    try {
      return await this.toolKit.screenshotBase64();
    } catch (_e) {
      return '';
    }
  }
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
