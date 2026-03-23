import type { AgentAction, ActionParameters, ActionResult } from '../agentspace/AgentSpace';
import { ToolKit } from '../toolkit/ToolKit';
import { VisiblePageContentA11yTree } from '../state_collector/a11y_tree/VisiblePageContentA11yTree';
import type { A11yTreeHitTarget } from '../state_collector/a11y_tree/VisiblePageContentA11yTree';

export type ToolCallStatus = 'SUCCESS' | 'FAILED';

export interface ToolCallResult {
  status: ToolCallStatus;
  error: string;
  screenshot_b64: string;
  uiTargetText: string;
  uiTargetIndex: number;
  action: AgentAction | null;
}

interface ToolCallArgs {
  point?: Array<number>;
  start?: Array<number>;
  end?: Array<number>;
  duration_ms?: number;
  seconds?: number;
  content?: string;
  app?: string;
  direction?: string;
  value?: string;
}

const DEFAULT_LONG_CLICK_MS: number = 1500;
const DEFAULT_SWIPE_MS: number = 300;
const DEFAULT_WAIT_SECONDS: number = 3;
const LOG_TAG: string = '[ToolCallDispatcher]';

export class ToolCallDispatcher {
  private toolKit: ToolKit;
  private waitAfterActionMs: number;

  constructor(toolKit: ToolKit, waitAfterActionMs?: number) {
    this.toolKit = toolKit;
    this.waitAfterActionMs = this.normalizeWaitMs(waitAfterActionMs);
  }

  async execute(name: string, argsJson: string): Promise<ToolCallResult> {
    const toolName = name.trim().toLowerCase();
    console.info(`${LOG_TAG} execute start name=${toolName} rawArgs=${argsJson}`);
    if (toolName.length === 0) {
      return { status: 'FAILED', error: 'empty_tool_name', screenshot_b64: '', uiTargetText: '', uiTargetIndex: 0, action: null };
    }
    const args = this.parseArgs(argsJson);
    if (!args) {
      const snapshot = await this.safeScreenshot();
      return {
        status: 'FAILED',
        error: 'invalid_arguments_json',
        screenshot_b64: snapshot,
        uiTargetText: '',
        uiTargetIndex: 0,
        action: null,
      };
    }
    if (toolName === 'screenshot') {
      const snapshot = await this.safeScreenshot();
      if (snapshot.length === 0) {
        return {
          status: 'FAILED',
          error: 'screenshot_failed',
          screenshot_b64: snapshot,
          uiTargetText: '',
          uiTargetIndex: 0,
          action: { name: 'screenshot' }
        };
      }
      return {
        status: 'SUCCESS',
        error: '',
        screenshot_b64: snapshot,
        uiTargetText: '',
        uiTargetIndex: 0,
        action: { name: 'screenshot' },
      };
    }

    let status: ToolCallStatus = 'SUCCESS';
    let error: string = '';
    let actionExecuted = false;
    let action: AgentAction | null = null;
    let uiTargetText: string = '';
    let uiTargetIndex: number = 0;
    try {
      action = this.buildAction(toolName, args);
      if (!action) {
        status = 'FAILED';
        error = 'invalid_arguments';
        console.warn(`${LOG_TAG} execute invalid action name=${toolName} parsedArgs=${JSON.stringify(args)}`);
      } else {
        console.info(`${LOG_TAG} execute action name=${toolName} action=${JSON.stringify(action)}`);
        const hit = await this.resolveUiTargetBestEffort(toolName, action);
        uiTargetText = hit.text;
        uiTargetIndex = hit.index;
        console.info(`${LOG_TAG} execute target name=${toolName} uiTargetText=${uiTargetText} uiTargetIndex=${uiTargetIndex}`);
        actionExecuted = true;
        const result = await this.toolKit.executeAction(action);
        console.info(`${LOG_TAG} execute result name=${toolName} status=${result.status} error=${result.error ?? ''}`);
        if (result.status !== 'SUCCESS') {
          status = 'FAILED';
          error = this.formatActionError(result);
        }
      }
    } catch (e) {
      status = 'FAILED';
      error = String(e);
      console.error(`${LOG_TAG} execute exception name=${toolName} error=${error}`);
    }

    if (actionExecuted && this.shouldWaitAfterAction(toolName)) {
      await sleep(this.waitAfterActionMs);
    }

    const snapshot = await this.safeScreenshot();
    if (snapshot.length === 0 && status === 'SUCCESS') {
      status = 'FAILED';
      error = error.length > 0 ? error : 'screenshot_failed';
    }
    console.info(
      `${LOG_TAG} execute done name=${toolName} status=${status} error=${error} screenshotLen=${snapshot.length} actionExecuted=${actionExecuted}`
    );
    return { status, error, screenshot_b64: snapshot, uiTargetText, uiTargetIndex, action };
  }

  private isScreenPointAction(toolName: string, action: AgentAction): boolean {
    if (toolName === 'click') {
      return true;
    }
    if (toolName === 'long_click') {
      return true;
    }
    if (toolName === 'set_scroll_value') {
      return true;
    }
    if (toolName === 'type') {
      const pt = action.parameters?.point;
      return Array.isArray(pt) && pt.length >= 2;
    }
    return false;
  }

  private async resolveUiTargetBestEffort(toolName: string, action: AgentAction): Promise<A11yTreeHitTarget> {
    if (!this.isScreenPointAction(toolName, action)) {
      return { text: '', index: 0 };
    }
    const pt = action.parameters?.point;
    if (!pt || pt.length < 2) {
      return { text: '', index: 0 };
    }
    const xNorm = Number(pt[0]);
    const yNorm = Number(pt[1]);
    if (!Number.isFinite(xNorm) || !Number.isFinite(yNorm)) {
      return { text: '', index: 0 };
    }
    try {
      const runtime = await this.toolKit.ensureVts();
      const dump = await this.toolKit.getVisiblePageContent(0);
      if (dump.trim().length === 0) {
        return { text: '', index: 0 };
      }
      const list = VisiblePageContentA11yTree.buildListFromDump(dump, runtime.widthPx, runtime.heightPx);
      const px = this.resolvePointPx(runtime.widthPx, runtime.heightPx, xNorm, yNorm);
      const xPx = px[0];
      const yPx = px[1];
      return VisiblePageContentA11yTree.hitTargetAtPoint(list, xPx, yPx);
    } catch (_e) {
      return { text: '', index: 0 };
    }
  }

  private resolvePointPx(widthPx: number, heightPx: number, x: number, y: number): Array<number> {
    const w = Math.max(1, Math.floor(widthPx));
    const h = Math.max(1, Math.floor(heightPx));
    const clampPx = (value: number, limit: number): number => {
      const safe = Number.isFinite(value) ? value : 0;
      const clamped = Math.min(Math.max(safe, 0), Math.max(0, limit - 1));
      return Math.round(clamped);
    };
    const ratioToPx = (value: number, limit: number): number => {
      const safe = Number.isFinite(value) ? value : 0;
      const clamped = Math.min(Math.max(safe, 0), 1);
      return Math.round(clamped * (Math.max(1, limit) - 1));
    };
    const normToPx1000 = (value: number, limit: number): number => {
      const safe = Number.isFinite(value) ? value : 0;
      const clamped = Math.min(Math.max(safe, 0), 1000);
      return Math.round((clamped / 1000) * (Math.max(1, limit) - 1));
    };

    const xPx = (x >= 0 && x <= 1) ? ratioToPx(x, w) : (x > 1000 ? clampPx(x, w) : normToPx1000(x, w));
    const yPx = (y >= 0 && y <= 1) ? ratioToPx(y, h) : (y > 1000 ? clampPx(y, h) : normToPx1000(y, h));
    return [xPx, yPx];
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
    if (name === 'set_scroll_value') {
      const point = this.readPoint(args.point);
      if (!point) {
        return null;
      }
      const direction = this.readText(args.direction).toLowerCase();
      if (direction !== 'up' && direction !== 'down') {
        return null;
      }
      const value = this.readText(args.value);
      if (value.length === 0) {
        return null;
      }
      const params: ActionParameters = {
        point,
        direction: direction as 'up' | 'down',
        value,
      };
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
      const parsed: ToolCallArgs = JSON.parse(trimmed) as ToolCallArgs;
      if (!parsed || typeof parsed !== 'object') {
        return null;
      }
      if (Array.isArray(parsed)) {
        return null;
      }
      return parsed;
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
