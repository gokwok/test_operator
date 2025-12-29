import type { common, Want } from '@kit.AbilityKit';

import { VTSBackend } from './VTSBackend';
import type { AppTarget } from './VTSBackend';

export enum AgentSpaceState {
  IDLE = 'IDLE',
  READY = 'READY',
  STOPPING = 'STOPPING',
  ERROR = 'ERROR',
}

export interface AgentSpaceRuntimeInfo {
  state: AgentSpaceState;
  viewerActive: boolean;
  surfaceAttached: boolean;
  screenId: number;
  displayId?: number;
  widthPx: number;
  heightPx: number;
}

export type ActionResultStatus = 'SUCCESS' | 'FAILED' | 'SKIPPED';

export interface ActionResult {
  status: ActionResultStatus;
  error?: string;
  output?: string;
}

export interface AgentAction {
  name: string;
  parameters?: ActionParameters;
}

export interface ActionParameters {
  point?: Array<number>;
  start_point?: Array<number>;
  end_point?: Array<number>;
  duration_ms?: number;
  seconds?: number;
  content?: string;
  app?: string;
  options?: Array<string>;
}

interface AbilityStarterContext {
  startAbility?: (w: Want) => Promise<void> | void;
}

const VIEWER_BUNDLE_NAME: string = 'com.huawei.aios.agent.operation';
const VIEWER_ABILITY_NAME: string = 'AgentSpaceViewerAbility';
const VIEWER_CLOSE_ACTION: string = 'closeViewer';

export class AgentSpace {
  private static instance: AgentSpace | null = null;
  private ctx: common.Context;
  private backend: VTSBackend;
  private state: AgentSpaceState = AgentSpaceState.IDLE;
  private viewerActive: boolean = false;
  private surfaceId: string = '';
  private surfaceAttached: boolean = false;

  static getInstance(ctx: common.Context): AgentSpace {
    if (!AgentSpace.instance) {
      AgentSpace.instance = new AgentSpace(ctx);
    } else {
      AgentSpace.instance.updateContext(ctx);
    }
    return AgentSpace.instance;
  }

  private constructor(ctx: common.Context) {
    this.ctx = ctx;
    this.backend = new VTSBackend(ctx as common.UIAbilityContext);
  }

  updateContext(ctx: common.Context): void {
    this.ctx = ctx;
    this.backend.updateContext(ctx as common.UIAbilityContext);
  }

  getRuntimeInfo(): AgentSpaceRuntimeInfo {
    const rt = this.backend.getRuntime();
    const info: AgentSpaceRuntimeInfo = {
      state: this.state,
      viewerActive: this.viewerActive,
      surfaceAttached: this.surfaceAttached,
      screenId: rt?.screenId ?? -1,
      displayId: rt?.displayId,
      widthPx: rt?.widthPx ?? -1,
      heightPx: rt?.heightPx ?? -1,
    };
    return info;
  }

  async ensureVts(): Promise<AgentSpaceRuntimeInfo> {
    try {
      await this.backend.create();
      this.state = AgentSpaceState.READY;
    } catch (e) {
      this.state = AgentSpaceState.ERROR;
      const err = e instanceof Error ? e : new Error(String(e));
      throw err;
    }
    return this.getRuntimeInfo();
  }

  async attachViewerSurface(surfaceId: string): Promise<void> {
    await this.ensureVts();
    await this.backend.attachSurface(surfaceId);
    this.surfaceId = surfaceId;
    this.surfaceAttached = true;
  }

  setViewerActive(active: boolean): void {
    this.viewerActive = active;
    if (!active) {
      this.surfaceId = '';
      this.surfaceAttached = false;
    }
  }

  async startApp(target: AppTarget): Promise<void> {
    await this.ensureVts();
    await this.backend.startApp(target);
  }

  async clickPx(x: number, y: number): Promise<void> {
    await this.ensureVts();
    await this.backend.clickPx(x, y);
  }

  async swipePx(startX: number, startY: number, endX: number, endY: number): Promise<void> {
    await this.ensureVts();
    await this.backend.swipePx(startX, startY, endX, endY);
  }

  async screenshotBase64(): Promise<string> {
    await this.ensureVts();
    return await this.backend.screenshotBase64();
  }

  async transferToMain(): Promise<number> {
    await this.ensureVts();
    return await this.backend.transferToMain();
  }

  async executeAction(action: AgentAction): Promise<ActionResult> {
    const name = (action.name ?? '').trim().toLowerCase();
    if (name.length === 0) {
      return { status: 'FAILED', error: 'empty_action_name' };
    }
    if (name === 'click') {
      return await this.handleClick(action.parameters);
    }
    if (name === 'double_click') {
      return await this.handleDoubleClick(action.parameters);
    }
    if (name === 'long_click') {
      return await this.handleLongClick(action.parameters);
    }
    if (name === 'swipe') {
      return await this.handleSwipe(action.parameters);
    }
    if (name === 'type') {
      return await this.handleType(action.parameters);
    }
    if (name === 'wait') {
      return await this.handleWait(action.parameters);
    }
    if (name === 'press_back') {
      return await this.handlePressBack();
    }
    if (name === 'press_enter') {
      return await this.handlePressEnter();
    }
    if (name === 'launch_app') {
      return await this.handleLaunchApp(action.parameters);
    }

    if (name === 'call_user' || name === 'interact' || name === 'task_over' || name === 'take_note' || name === 'finish') {
      return { status: 'SKIPPED', error: `not_implemented:${name}` };
    }

    return { status: 'FAILED', error: `unknown_action:${name}` };
  }

  async destroy(): Promise<void> {
    this.state = AgentSpaceState.STOPPING;
    try {
      await this.closeViewerBestEffort();
      await this.backend.destroy();
    } finally {
      this.state = AgentSpaceState.IDLE;
      this.viewerActive = false;
      this.surfaceId = '';
      this.surfaceAttached = false;
    }
  }

  private async handleClick(params?: ActionParameters): Promise<ActionResult> {
    const pt = this.readPoint(params, 'point');
    if (!pt) {
      return { status: 'FAILED', error: 'missing_point' };
    }
    const rt = await this.ensureVts();
    const x = this.normToPx(rt.widthPx, pt[0]);
    const y = this.normToPx(rt.heightPx, pt[1]);
    await this.backend.clickPx(x, y);
    return { status: 'SUCCESS' };
  }

  private async handleDoubleClick(params?: ActionParameters): Promise<ActionResult> {
    const pt = this.readPoint(params, 'point');
    if (!pt) {
      return { status: 'FAILED', error: 'missing_point' };
    }
    const rt = await this.ensureVts();
    const x = this.normToPx(rt.widthPx, pt[0]);
    const y = this.normToPx(rt.heightPx, pt[1]);
    await this.backend.clickPx(x, y);
    await sleep(80);
    await this.backend.clickPx(x, y);
    return { status: 'SUCCESS' };
  }

  private async handleLongClick(params?: ActionParameters): Promise<ActionResult> {
    const pt = this.readPoint(params, 'point');
    if (!pt) {
      return { status: 'FAILED', error: 'missing_point' };
    }
    const duration = this.readNumber(params?.duration_ms, 1500);
    const rt = await this.ensureVts();
    const x = this.normToPx(rt.widthPx, pt[0]);
    const y = this.normToPx(rt.heightPx, pt[1]);
    await this.backend.longClickPx(x, y, duration);
    return { status: 'SUCCESS' };
  }

  private async handleSwipe(params?: ActionParameters): Promise<ActionResult> {
    const start = this.readPoint(params, 'start_point');
    const end = this.readPoint(params, 'end_point');
    if (!start || !end) {
      return { status: 'FAILED', error: 'missing_swipe_points' };
    }
    const duration = this.readNumber(params?.duration_ms, 300);
    const rt = await this.ensureVts();
    const sx = this.normToPx(rt.widthPx, start[0]);
    const sy = this.normToPx(rt.heightPx, start[1]);
    const ex = this.normToPx(rt.widthPx, end[0]);
    const ey = this.normToPx(rt.heightPx, end[1]);
    await this.backend.swipePx(sx, sy, ex, ey, duration);
    return { status: 'SUCCESS' };
  }

  private async handleType(params?: ActionParameters): Promise<ActionResult> {
    const content = params?.content ?? '';
    if (content.length === 0) {
      return { status: 'FAILED', error: 'empty_content' };
    }
    const pt = this.readPoint(params, 'point');
    if (pt) {
      const rt = await this.ensureVts();
      const x = this.normToPx(rt.widthPx, pt[0]);
      const y = this.normToPx(rt.heightPx, pt[1]);
      await this.backend.clickPx(x, y);
      await sleep(120);
    } else {
      await this.ensureVts();
    }
    let sendEnter = false;
    let text = content;
    if (text.endsWith('\n')) {
      sendEnter = true;
      text = text.slice(0, -1);
    }
    await this.backend.inputText(text, sendEnter);
    return { status: 'SUCCESS' };
  }

  private async handleWait(params?: ActionParameters): Promise<ActionResult> {
    const seconds = this.readNumber(params?.seconds, 1);
    await sleep(Math.max(0, Math.floor(seconds * 1000)));
    return { status: 'SUCCESS' };
  }

  private async handlePressEnter(): Promise<ActionResult> {
    await this.ensureVts();
    await this.backend.pressEnter();
    return { status: 'SUCCESS' };
  }

  private async handlePressBack(): Promise<ActionResult> {
    return { status: 'SKIPPED', error: 'press_back_not_implemented' };
  }

  private async handleLaunchApp(params?: ActionParameters): Promise<ActionResult> {
    const name = params?.app ?? '';
    const target = this.resolveAppTarget(name);
    if (!target) {
      return { status: 'FAILED', error: 'app_not_mapped' };
    }
    await this.startApp(target);
    return { status: 'SUCCESS' };
  }

  private resolveAppTarget(name: string): AppTarget | null {
    const trimmed = name.trim();
    if (trimmed.length === 0) {
      return null;
    }
    const slashIndex = trimmed.indexOf('/');
    if (slashIndex > 0 && slashIndex < trimmed.length - 1) {
      const bundleName = trimmed.slice(0, slashIndex);
      const abilityName = trimmed.slice(slashIndex + 1);
      return { bundleName, abilityName };
    }
    if (trimmed.indexOf('.') >= 0) {
      return { bundleName: trimmed, abilityName: 'EntryAbility' };
    }
    return null;
  }

  private readPoint(params: ActionParameters | undefined, key: 'point' | 'start_point' | 'end_point'): Array<number> | null {
    if (!params) {
      return null;
    }
    let raw: Array<number> | undefined;
    if (key === 'point') {
      raw = params.point;
    } else if (key === 'start_point') {
      raw = params.start_point;
    } else {
      raw = params.end_point;
    }
    if (!raw || raw.length < 2) {
      return null;
    }
    const a = Number(raw[0]);
    const b = Number(raw[1]);
    if (!Number.isFinite(a) || !Number.isFinite(b)) {
      return null;
    }
    return [a, b];
  }

  private readNumber(value: number | undefined, fallback: number): number {
    if (typeof value !== 'number' || !Number.isFinite(value)) {
      return fallback;
    }
    return value;
  }

  private normToPx(limit: number, value: number): number {
    const clamped = Math.min(Math.max(value, 0), 1000);
    const max = Math.max(1, limit);
    const px = Math.round((clamped / 1000) * (max - 1));
    return px < 0 ? 0 : px;
  }

  private async closeViewerBestEffort(): Promise<void> {
    if (!this.viewerActive) {
      return;
    }
    const starterCtx: AbilityStarterContext = this.ctx as AbilityStarterContext;
    const starter = starterCtx.startAbility;
    if (typeof starter !== 'function') {3
      this.viewerActive = false;
      this.surfaceId = '';
      this.surfaceAttached = false;
      return;
    }

    const want: Want = {
      bundleName: VIEWER_BUNDLE_NAME,
      abilityName: VIEWER_ABILITY_NAME,
      action: VIEWER_CLOSE_ACTION,
    };
    try {
      await starter(want);
    } catch {
      // Ignore viewer close failures (best-effort).
    }
  }
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
