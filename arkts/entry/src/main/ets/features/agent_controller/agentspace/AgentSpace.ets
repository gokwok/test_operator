import type { common } from '@kit.AbilityKit';
import photoAccessHelper from '@ohos.file.photoAccessHelper';
import fs from '@ohos.file.fs';
import image from '@ohos.multimedia.image';
import { VTSBackend } from './VTSBackend';
import type { AppTarget, StartAbilityInvoker, VTSBackendPort } from './VTSBackend';
import { VisiblePageContentA11yTree } from '../state_collector/a11y_tree/VisiblePageContentA11yTree';

const EFFECT_LOG_TAG: string = '[AS_EFFECT]';

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
  appLaunched: boolean;
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
  key?: string;
  keys?: Array<string>;
  direction?: 'up' | 'down';
  value?: string;
}

export type AgentSpaceViewerMode = 'display' | 'control';

export type AgentSpacePointerAction = 'down' | 'move' | 'up' | 'cancel';

export interface AgentSpacePointerEvent {
  action: AgentSpacePointerAction;
  xNorm: number;
  yNorm: number;
  pointerId?: number;
}

export interface AgentSpaceViewerTapEffect {
  kind: 'tap';
  xNorm: number;
  yNorm: number;
}

export interface AgentSpaceViewerLongPressEffect {
  kind: 'long_press';
  xNorm: number;
  yNorm: number;
  durationMs?: number;
}

export interface AgentSpaceViewerSwipeEffect {
  kind: 'swipe';
  startXNorm: number;
  startYNorm: number;
  endXNorm: number;
  endYNorm: number;
  durationMs?: number;
}

export type AgentSpaceViewerEffect =
  AgentSpaceViewerTapEffect | AgentSpaceViewerLongPressEffect | AgentSpaceViewerSwipeEffect;

export interface AgentSpaceViewerEffectBatch {
  sequence: number;
  effects: Array<AgentSpaceViewerEffect>;
}

export interface AgentSpaceHost {
  startViewer(mode: AgentSpaceViewerMode): Promise<void>;
  closeViewer(): Promise<void>;
}

export type VTSBackendFactory = (invoker: StartAbilityInvoker) => VTSBackendPort;

export class AgentSpace {
  private backend: VTSBackendPort;
  private host: AgentSpaceHost | null = null;
  private state: AgentSpaceState = AgentSpaceState.IDLE;
  private viewerActive: boolean = false;
  private surfaceId: string = '';
  private surfaceAttached: boolean = false;
  private viewerMode: AgentSpaceViewerMode = 'display';
  private viewerEffectSequence: number = 0;
  private viewerEffects: Array<AgentSpaceViewerEffect> = [];
  private appLaunched: boolean = false;
  private ctx: common.Context | null = null;

  constructor(
    startAbilityInvoker: StartAbilityInvoker,
    backendFactory?: VTSBackendFactory,
    host?: AgentSpaceHost,
    ctx?: common.Context
  ) {
    this.backend = backendFactory ? backendFactory(startAbilityInvoker) : new VTSBackend(startAbilityInvoker);
    if (host) {
      this.host = host;
    }
    this.ctx = ctx ?? null;
  }

  updateStartAbility(startAbilityInvoker: StartAbilityInvoker): void {
    this.backend.updateStartAbility(startAbilityInvoker);
  }

  setHost(host: AgentSpaceHost | null): void {
    this.host = host;
  }

  isAppLaunched(): boolean {
    return this.appLaunched;
  }

  get_current_app_bundle(): AppTarget | null {
    return this.backend.get_current_app_bundle();
  }

  async getRuntimeInfo(): Promise<AgentSpaceRuntimeInfo> {
    const rt = this.backend.getRuntime();
    const info: AgentSpaceRuntimeInfo = {
      state: this.state,
      viewerActive: this.viewerActive,
      surfaceAttached: this.surfaceAttached,
      appLaunched: this.appLaunched,
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
    return await this.getRuntimeInfo();
  }

  async attachViewerSurface(surfaceId: string): Promise<void> {
    await this.ensureVts();
    await this.backend.attachSurface(surfaceId);
    this.surfaceId = surfaceId;
    this.surfaceAttached = true;
  }

  async setViewerActive(active: boolean): Promise<void> {
    this.viewerActive = active;
    if (!active) {
      this.surfaceId = '';
      this.surfaceAttached = false;
      this.viewerEffectSequence = 0;
      this.viewerEffects = [];
    }
  }

  async setViewerMode(mode: AgentSpaceViewerMode): Promise<void> {
    if (this.viewerMode === mode) {
      return;
    }
    this.viewerMode = mode;
    try {
      this.backend.setUserTakeover(mode === 'control');
    } catch (e) {
      console.warn(`[AgentSpace] setUserTakeover failed: ${String(e)}`);
    }
  }

  async getViewerMode(): Promise<AgentSpaceViewerMode> {
    return this.viewerMode;
  }

  async sendViewerEvent(event: AgentSpacePointerEvent): Promise<void> {
    await this.handleViewerPointerInternal(event);
  }

  async pullViewerEffects(lastSequence: number): Promise<AgentSpaceViewerEffectBatch | null> {
    if (this.viewerEffectSequence === lastSequence || this.viewerEffects.length === 0) {
      return null;
    }
    const effects = this.viewerEffects;
    this.viewerEffects = [];
    const batch: AgentSpaceViewerEffectBatch = {
      sequence: this.viewerEffectSequence,
      effects,
    };
    console.info(`${EFFECT_LOG_TAG} flush effects last=${lastSequence} new=${batch.sequence} count=${effects.length}`);
    return batch;
  }

  async startViewer(mode: AgentSpaceViewerMode): Promise<void> {
    this.viewerMode = mode;
    try {
      this.backend.setUserTakeover(mode === 'control');
    } catch (e) {
      console.warn(`[AgentSpace] setUserTakeover failed: ${String(e)}`);
    }
    const host = this.host;
    if (!host) {
      return;
    }
    await host.startViewer(mode);
  }

  async closeViewer(): Promise<void> {
    const host = this.host;
    if (!host) {
      this.viewerActive = false;
      this.surfaceId = '';
      this.surfaceAttached = false;
      return;
    }
    await host.closeViewer();
  }

  async startApp(target: AppTarget): Promise<void> {
    await this.ensureVts();
    await this.backend.startApp(target);
    this.appLaunched = true;
  }

  async clickPx(x: number, y: number): Promise<void> {
    const rt = await this.ensureVts();
    await this.backend.clickPx(x, y);
    this.emitViewerTapFromPx(x, y, rt);
  }

  async swipePx(startX: number, startY: number, endX: number, endY: number): Promise<void> {
    const rt = await this.ensureVts();
    await this.backend.swipePx(startX, startY, endX, endY);
    this.emitViewerSwipeFromPx(startX, startY, endX, endY, undefined, rt);
  }

  async screenshotBase64(): Promise<string> {
    await this.ensureVts();
    return await this.backend.screenshotBase64();
  }

  async screenshotLongBase64(): Promise<string> {
    await this.ensureVts();
    return await this.backend.screenshotLongBase64();
  }

  async getVisiblePageContent(windowId: number): Promise<string> {
    await this.ensureVts();
    return await this.backend.getVisiblePageContent(windowId);
  }

  async getPageContent(windowId: number): Promise<string> {
    await this.ensureVts();
    return await this.backend.getPageContent(windowId);
  }

  async transferToMain(): Promise<number> {
    await this.ensureVts();
    const transferred = await this.backend.transferToMain();
    if (transferred > 0) {
      this.appLaunched = false;
    }
    return transferred;
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
    if (name === 'set_scroll_value') {
      return await this.handleSetScrollValue(action.parameters);
    }
    if (name === 'press_back') {
      return await this.handlePressBack();
    }
    if (name === 'press_enter') {
      return await this.handlePressEnter();
    }
    if (name === 'press_backspace') {
      return await this.handlePressBackspace();
    }
    if (name === 'press_key') {
      return await this.handlePressKey(action.parameters);
    }
    if (name === 'press_combo_key') {
      return await this.handlePressComboKey(action.parameters);
    }
    if (name === 'request_focus') {
      return await this.handleRequestFocus();
    }
    if (name === 'screenshot') {
      return await this.handleScreenshot();
    }
    if (name === 'launch_app') {
      return await this.handleLaunchApp(action.parameters);
    }

    if (name === 'call_user' || name === 'interact' || name === 'take_over' || name === 'take_note' || name === 'finish') {
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
      this.appLaunched = false;
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
    this.emitViewerTapNorm(pt[0], pt[1]);
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
    this.emitViewerTapNorm(pt[0], pt[1]);
    await sleep(80);
    await this.backend.clickPx(x, y);
    this.emitViewerTapNorm(pt[0], pt[1]);
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
    this.emitViewerLongPressNorm(pt[0], pt[1], duration);
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
    this.emitViewerSwipeNorm(start[0], start[1], end[0], end[1], duration);
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
      this.emitViewerTapNorm(pt[0], pt[1]);
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

  private async handleSetScrollValue(params?: ActionParameters): Promise<ActionResult> {
    const pt = this.readPoint(params, 'point');
    if (!pt) {
      return { status: 'FAILED', error: 'missing_point' };
    }
    const expected = normalizeUiText(params?.value ?? '');
    if (expected.length === 0) {
      return { status: 'FAILED', error: 'empty_value' };
    }
    const rawDir = String(params?.direction ?? '').trim().toLowerCase();
    if (rawDir !== 'up' && rawDir !== 'down') {
      return { status: 'FAILED', error: 'invalid_direction' };
    }
    const valueDirection: 'up' | 'down' = rawDir;

    const runtime = await this.ensureVts();
    const screenWidth = Math.max(1, Math.floor(runtime.widthPx));
    const screenHeight = Math.max(1, Math.floor(runtime.heightPx));
    if (screenWidth <= 1 || screenHeight <= 1) {
      return { status: 'FAILED', error: 'invalid_screen_size' };
    }

    const xPx = this.normToPx(screenWidth, pt[0]);
    const yPx = this.normToPx(screenHeight, pt[1]);

    const startedAt = Date.now();
    const maxSteps = 40;
    const timeoutMs = 15000;
    const settleDelayMs = 220;
    const swipeDurationMs = 180;
    const stuckThreshold = 3;
    const emptyThreshold = 6;

    const clampY = (value: number): number => {
      const max = Math.max(0, screenHeight - 1);
      return Math.min(Math.max(0, Math.round(value)), max);
    };

    const readHitText = async (): Promise<string> => {
      const dump = await this.getVisiblePageContent(0);
      if (dump.trim().length === 0) {
        return '';
      }
      const list = VisiblePageContentA11yTree.buildListFromDump(dump, screenWidth, screenHeight);
      const hit = VisiblePageContentA11yTree.hitTargetAtPoint(list, xPx, yPx);
      return normalizeUiText(hit.text ?? '');
    };

    const truncate = (text: string, limit: number): string => {
      if (text.length <= limit) {
        return text;
      }
      return `${text.slice(0, limit)}...`;
    };

    const fingerSwipeUp = valueDirection === 'down';
    let swipeDistancePx = Math.max(12, Math.min(120, Math.round(screenHeight * 0.035)));
    const maxSwipeDistancePx = Math.max(swipeDistancePx, Math.round(screenHeight * 0.18));

    let currentText = await readHitText();
    if (currentText === expected) {
      return { status: 'SUCCESS', output: currentText };
    }

    let consecutiveStuck = 0;
    let consecutiveEmpty = currentText.length === 0 ? 1 : 0;

    for (let step = 0; step < maxSteps; step += 1) {
      if (Date.now() - startedAt > timeoutMs) {
        const last = currentText.length > 0 ? truncate(currentText, 80) : '';
        return { status: 'FAILED', error: last.length > 0 ? `set_scroll_value_timeout:last=${last}` : 'set_scroll_value_timeout' };
      }

      const half = Math.max(2, Math.floor(swipeDistancePx / 2));
      const startY = clampY(fingerSwipeUp ? (yPx + half) : (yPx - half));
      const endY = clampY(fingerSwipeUp ? (yPx - half) : (yPx + half));
      if (startY === endY) {
        return { status: 'FAILED', error: 'invalid_swipe_range' };
      }

      await this.backend.swipePx(xPx, startY, xPx, endY, swipeDurationMs);
      this.emitViewerSwipeFromPx(xPx, startY, xPx, endY, swipeDurationMs, runtime);
      await sleep(settleDelayMs);

      const updatedText = await readHitText();
      if (updatedText === expected) {
        return { status: 'SUCCESS', output: updatedText };
      }

      if (updatedText.length === 0) {
        consecutiveEmpty += 1;
        if (consecutiveEmpty >= emptyThreshold) {
          return { status: 'FAILED', error: 'set_scroll_value_empty_hit_text' };
        }
      } else {
        consecutiveEmpty = 0;
      }

      if (updatedText === currentText) {
        consecutiveStuck += 1;
        if (consecutiveStuck >= stuckThreshold) {
          if (swipeDistancePx < maxSwipeDistancePx) {
            swipeDistancePx = Math.min(maxSwipeDistancePx, Math.max(swipeDistancePx + 1, Math.round(swipeDistancePx * 1.6)));
            consecutiveStuck = 0;
          } else {
            const last = currentText.length > 0 ? truncate(currentText, 80) : '';
            return { status: 'FAILED', error: last.length > 0 ? `set_scroll_value_stuck:last=${last}` : 'set_scroll_value_stuck' };
          }
        }
      } else {
        consecutiveStuck = 0;
      }

      currentText = updatedText;
    }

    const last = currentText.length > 0 ? truncate(currentText, 80) : '';
    return { status: 'FAILED', error: last.length > 0 ? `set_scroll_value_max_steps:last=${last}` : 'set_scroll_value_max_steps' };
  }

  private async handlePressEnter(): Promise<ActionResult> {
    await this.ensureVts();
    await this.backend.pressEnter();
    return { status: 'SUCCESS' };
  }

  private async handlePressBackspace(): Promise<ActionResult> {
    await this.ensureVts();
    await this.backend.pressBackspace();
    return { status: 'SUCCESS' };
  }

  private async handlePressBack(): Promise<ActionResult> {
    await this.ensureVts();
    await this.backend.pressBack();
    return { status: 'SUCCESS' };
  }

  private async handlePressKey(params?: ActionParameters): Promise<ActionResult> {
    const key = (params?.key ?? '').trim();
    await this.ensureVts();
    const ok = await this.backend.pressKey(key);
    return ok ? { status: 'SUCCESS' } : { status: 'FAILED', error: 'press_key_failed' };
  }

  private async handlePressComboKey(params?: ActionParameters): Promise<ActionResult> {
    const keys: Array<string> = params?.keys ?? [];
    await this.ensureVts();
    const ok = await this.backend.pressComboKey(keys);
    return ok ? { status: 'SUCCESS' } : { status: 'FAILED', error: 'press_combo_key_failed' };
  }

  private async handleRequestFocus(): Promise<ActionResult> {
    await this.ensureVts();
    const ok = await this.backend.requestFocus();
    return ok ? { status: 'SUCCESS' } : { status: 'FAILED', error: 'request_focus_failed' };
  }

  private async handleScreenshot(): Promise<ActionResult> {
    await this.ensureVts();
    const pixelMap = await this.backend.screenshotPixelMap();
    if (!pixelMap) {
      return { status: 'FAILED', error: 'screenshot_failed' };
    }
    const saved = await this.savePixelMapToAlbum(pixelMap);
    if (!saved) {
      return { status: 'FAILED', error: 'save_failed' };
    }
    return { status: 'SUCCESS' };
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

  private async savePixelMapToAlbum(pixelMap: image.PixelMap): Promise<boolean> {
    const ctx = this.ctx;
    if (!ctx) {
      return false;
    }
    const helper = photoAccessHelper.getPhotoAccessHelper(ctx);
    const displayName = `agent_screenshot_${Date.now()}.jpg`;
    let fd = -1;
    try {
      const asset = await helper.createAsset(displayName);
      fd = await asset.open('rw');
      const packer = image.createImagePacker();
      const buffer: ArrayBuffer = await packer.packing(pixelMap, {
        format: 'image/jpeg',
        quality: 90,
        bufferSize: 50 * 1024 * 1024,
      });
      await fs.write(fd, buffer);
      return true;
    } catch (_e) {
      return false;
    } finally {
      if (fd >= 0) {
        try {
          await fs.close(fd);
        } catch (_e) {
          // ignore
        }
      }
    }
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

  private async handleViewerPointerInternal(event: AgentSpacePointerEvent): Promise<void> {
    if (!event) {
      return;
    }
    const action = event.action;
    if (action !== 'down' && action !== 'move' && action !== 'up' && action !== 'cancel') {
      return;
    }
    const rt = await this.ensureVts();
    const x = this.ratioToPx(rt.widthPx, event.xNorm);
    const y = this.ratioToPx(rt.heightPx, event.yNorm);
    const pointerId = typeof event.pointerId === 'number' ? event.pointerId : 0;
    this.backend.injectPointer(action, x, y, pointerId);
  }

  private pushViewerEffect(effect: AgentSpaceViewerEffect): void {
    if (!this.viewerActive) {
      console.warn(`${EFFECT_LOG_TAG} drop effect: viewer inactive payload=${JSON.stringify(effect)}`);
      return;
    }
    this.viewerEffectSequence += 1;
    this.viewerEffects = [...this.viewerEffects, effect];
    console.info(`${EFFECT_LOG_TAG} queued effect seq=${this.viewerEffectSequence} payload=${JSON.stringify(effect)}`);
  }

  private emitViewerTapNorm(xNorm: number, yNorm: number): void {
    const effect: AgentSpaceViewerTapEffect = {
      kind: 'tap',
      xNorm: this.clampNorm(xNorm),
      yNorm: this.clampNorm(yNorm),
    };
    this.pushViewerEffect(effect);
  }

  private emitViewerLongPressNorm(xNorm: number, yNorm: number, durationMs: number): void {
    const effect: AgentSpaceViewerLongPressEffect = {
      kind: 'long_press',
      xNorm: this.clampNorm(xNorm),
      yNorm: this.clampNorm(yNorm),
    };
    if (Number.isFinite(durationMs)) {
      effect.durationMs = Math.max(0, Math.floor(durationMs));
    }
    this.pushViewerEffect(effect);
  }

  private emitViewerSwipeNorm(
    startXNorm: number,
    startYNorm: number,
    endXNorm: number,
    endYNorm: number,
    durationMs: number | undefined
  ): void {
    const effect: AgentSpaceViewerSwipeEffect = {
      kind: 'swipe',
      startXNorm: this.clampNorm(startXNorm),
      startYNorm: this.clampNorm(startYNorm),
      endXNorm: this.clampNorm(endXNorm),
      endYNorm: this.clampNorm(endYNorm),
    };
    if (typeof durationMs === 'number' && Number.isFinite(durationMs)) {
      effect.durationMs = Math.max(0, Math.floor(durationMs));
    }
    this.pushViewerEffect(effect);
  }

  private emitViewerTapFromPx(x: number, y: number, rt: AgentSpaceRuntimeInfo): void {
    const xNorm = this.pxToNorm(rt.widthPx, x);
    const yNorm = this.pxToNorm(rt.heightPx, y);
    this.emitViewerTapNorm(xNorm, yNorm);
  }

  private emitViewerSwipeFromPx(
    startX: number,
    startY: number,
    endX: number,
    endY: number,
    durationMs: number | undefined,
    rt: AgentSpaceRuntimeInfo
  ): void {
    const sxNorm = this.pxToNorm(rt.widthPx, startX);
    const syNorm = this.pxToNorm(rt.heightPx, startY);
    const exNorm = this.pxToNorm(rt.widthPx, endX);
    const eyNorm = this.pxToNorm(rt.heightPx, endY);
    this.emitViewerSwipeNorm(sxNorm, syNorm, exNorm, eyNorm, durationMs);
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

  private ratioToPx(limit: number, ratio: number): number {
    if (!Number.isFinite(ratio)) {
      return 0;
    }
    const clamped = Math.min(Math.max(ratio, 0), 1);
    const max = Math.max(1, limit);
    const px = Math.round(clamped * (max - 1));
    return px < 0 ? 0 : px;
  }

  private pxToNorm(limit: number, value: number): number {
    if (!Number.isFinite(value) || limit <= 1) {
      return 0;
    }
    const max = Math.max(1, limit);
    const ratio = value / (max - 1);
    return this.clampNorm(ratio * 1000);
  }

  private clampNorm(value: number): number {
    if (!Number.isFinite(value)) {
      return 0;
    }
    return Math.min(1000, Math.max(0, Math.round(value)));
  }

  private async closeViewerBestEffort(): Promise<void> {
    if (!this.viewerActive) {
      return;
    }
    const host = this.host;
    if (!host) {
      this.viewerActive = false;
      this.surfaceId = '';
      this.surfaceAttached = false;
      return;
    }
    try {
      await host.closeViewer();
    } catch {
      // Ignore viewer close failures (best-effort).
    }
  }
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function normalizeUiText(value: string): string {
  const raw = value.trim();
  if (raw.length === 0) {
    return '';
  }
  let out = '';
  let lastSpace = false;
  for (let i = 0; i < raw.length; i += 1) {
    const ch = raw.charAt(i);
    const isSpace = ch === ' ' || ch === '\n' || ch === '\t' || ch === '\r';
    if (isSpace) {
      if (!lastSpace) {
        out += ' ';
        lastSpace = true;
      }
      continue;
    }
    out += ch;
    lastSpace = false;
  }
  return out.trim();
}
