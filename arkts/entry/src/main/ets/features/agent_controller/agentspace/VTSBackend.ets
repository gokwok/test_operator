import type { StartOptions, Want } from '@kit.AbilityKit';
import { display, screen, screenshot } from '@kit.ArkUI';
import { BusinessError, systemDateTime } from '@kit.BasicServicesKit';
import image from '@ohos.multimedia.image';
import util from '@ohos.util';
import * as AppController from '@hms.ai.appController';
import { WallpaperState } from '../state_collector/WallpaperState';
import { Action, KeyCode } from '@kit.InputKit';
import { resolveCommonKeyCode } from './KeyCodeResolver';

const LOG_TAG: string = '[VTSBackend]';

const POINTER_ACTION_CANCEL: number = 1;
const POINTER_ACTION_DOWN: number = 2;
const POINTER_ACTION_MOVE: number = 3;
const POINTER_ACTION_UP: number = 4;

const KEY_ACTION_CANCEL: number = 1;
const KEY_ACTION_DOWN: number = 2;
const KEY_ACTION_UP: number = 3;

const IME_READY_WAIT_MS: number = 1200;
const IME_RECONNECT_WAIT_MS: number = 1200;
const IME_POLL_INTERVAL_MS: number = 80;

const LONG_SCREENSHOT_DEFAULT_SHOTS: number = 14;
const LONG_SCREENSHOT_MIN_OVERLAP_RATIO: number = 0.6;
const LONG_SCREENSHOT_SWIPE_START_Y_RATIO: number = 0.8;
const LONG_SCREENSHOT_SWIPE_DURATION_MS: number = 700;
const LONG_SCREENSHOT_SETTLE_DELAY_MS: number = 650;
const LONG_SCREENSHOT_STOP_ON_STABLE_COUNT: number = 1;
const LONG_SCREENSHOT_HASH_GRID_SIZE: number = 16;
const LONG_SCREENSHOT_SAME_BLOCK_RATIO: number = 0.75;

export type VTSBackendPointerAction = 'down' | 'move' | 'up' | 'cancel';
type PointerAction = VTSBackendPointerAction;

let lastImeStatus: number = -1;
let lastCallingWindowId: number = -1;
let lastImeSignalAtMs: number = 0;

enum defaultInputMethodConnId {
  UNREGISTER_VIRTUALSCREEN = 0,
  REGISTER_VIRTUALSCREEN = 1,
  UNREGISTER_MAINSCREEN = 2,
  REGISTER_MAINSCREEN = 3,
  UNREGISTER_NOTIFY_ONLY_MAIN = 4,
  REGISTER_NOTIFY_ONLY_MAIN = 5,
}

enum defaultInputMethodOperatorId {
  INSERT = 0,
  DELETE_FORWARD = 1,
  DELETE_BACKWARD = 2,
  MOVE_CURSOR = 3,
  ENTER = 4,
}

function onInputStatusFunc(unused: undefined, status: number): void {
  void unused;
  lastImeStatus = status;
  lastImeSignalAtMs = Date.now();
}

function onSetCallingWindowFunc(unused: undefined, windowId: number): void {
  void unused;
  lastCallingWindowId = windowId;
  lastImeSignalAtMs = Date.now();
}

function onSelectionChangeFunc(unused: undefined, data: AppController.onSelChgParam): void {
  void unused;
  void data;
}

function onTextChangeFunc(unused: undefined, data: string): void {
  void unused;
  void data;
}

function onFunctionKeyFunc(unused: undefined, funcKey: number): void {
  void unused;
  void funcKey;
}

export interface AppTarget {
  bundleName: string;
  abilityName: string;
}

export type StartAbilityInvoker = (want: Want, options?: StartOptions) => Promise<void>;

export interface VTSRuntimeInfo {
  screenId: number;
  displayId?: number;
  widthPx: number;
  heightPx: number;
  density: number;
  screenName: string;
}

export interface VTSBackendPort {
  updateStartAbility(invoker: StartAbilityInvoker): void;
  getRuntime(): VTSRuntimeInfo | null;
  get_current_app_bundle(): AppTarget | null;
  getVisiblePageContent(windowId: number): Promise<string>;
  getPageContent(windowId: number): Promise<string>;
  // When `active=true`, the user is taking over manual control so we should allow the normal IME to pop up.
  // When `active=false`, agent input relies on the proxy IME, so we should (re)register it.
  setUserTakeover(active: boolean): void;
  create(): Promise<VTSRuntimeInfo>;
  attachSurface(surfaceId: string): Promise<void>;
  startApp(target: AppTarget): Promise<void>;
  clickPx(x: number, y: number): Promise<void>;
  longClickPx(x: number, y: number, durationMs: number): Promise<void>;
  swipePx(startX: number, startY: number, endX: number, endY: number, durationMs?: number): Promise<void>;
  inputText(text: string, sendEnter: boolean): Promise<void>;
  pressEnter(): Promise<void>;
  pressBackspace(): Promise<void>;
  pressKey(code: number | string): Promise<boolean>;
  pressComboKey(codes: Array<number | string>): Promise<boolean>;
  pressBack(): Promise<void>;
  requestFocus(): Promise<boolean>;
  screenshotPixelMap(): Promise<image.PixelMap | null>;
  screenshotBase64(): Promise<string>;
  screenshotLongPixelMap(): Promise<Array<image.PixelMap>>;
  screenshotLongBase64(): Promise<string>;
  transferToMain(): Promise<number>;
  destroy(): Promise<void>;
  injectPointer(action: VTSBackendPointerAction, x: number, y: number, pointerId: number): void;
}

interface ScreenshotImageSize {
  width: number;
  height: number;
}

interface ScreenWithDisplayId extends screen.Screen {
  displayId?: number;
}

interface AppControllerFocusApi {
  requestFocus?: (targetWindowId: number) => boolean;
}

export class VTSBackend implements VTSBackendPort {
  private startAbilityInvoker: StartAbilityInvoker;
  private runtime: VTSRuntimeInfo | null = null;
  private lastTarget: AppTarget | null = null;
  private imeRegistered: boolean = false;
  private userTakeoverActive: boolean = false;
  private hasLaunchedApp: boolean = false;

  constructor(startAbilityInvoker: StartAbilityInvoker) {
    this.startAbilityInvoker = startAbilityInvoker;
  }

  updateStartAbility(invoker: StartAbilityInvoker): void {
    this.startAbilityInvoker = invoker;
  }

  getRuntime(): VTSRuntimeInfo | null {
    return this.runtime;
  }

  get_current_app_bundle(): AppTarget | null {
    return this.lastTarget;
  }

  async getVisiblePageContent(windowId: number): Promise<string> {
    const resolvedWindowId = this.resolveWindowIdBestEffort(windowId);
    if (resolvedWindowId <= 0) {
      return '';
    }
    try {
      return await AppController.getVisiblePageContent(resolvedWindowId);
    } catch (e) {
      console.warn(`${LOG_TAG} getVisiblePageContent failed: ${String(e)}`);
      return '';
    }
  }

  async getPageContent(windowId: number): Promise<string> {
    const resolvedWindowId = this.resolveWindowIdBestEffort(windowId);
    if (resolvedWindowId <= 0) {
      return '';
    }
    try {
      return await AppController.getPageContent(resolvedWindowId);
    } catch (e) {
      console.warn(`${LOG_TAG} getPageContent failed: ${String(e)}`);
      return '';
    }
  }

  setUserTakeover(active: boolean): void {
    const normalized = active === true;
    if (this.userTakeoverActive === normalized) {
      return;
    }
    this.userTakeoverActive = normalized;
    this.applyImePolicyForTakeover();
  }

  async create(): Promise<VTSRuntimeInfo> {
    if (this.runtime) {
      const displayId = this.resolveDisplayId(this.runtime);
      if (this.userTakeoverActive) {
        if (this.imeRegistered) {
          this.unregisterImeBestEffort(displayId);
          this.imeRegistered = false;
        }
      } else if (!this.imeRegistered) {
        this.imeRegistered = this.registerImeBestEffort(displayId);
      }
      return this.runtime;
    }

    let defaultDisplay: display.Display;
    try {
      defaultDisplay = display.getDefaultDisplaySync();
    } catch (e) {
      const err = e as BusinessError;
      throw new Error(`getDefaultDisplaySync failed. Code:${err.code}, message:${err.message}`);
    }

    /*
     * The names for individually grouped screens are hardcoded in the code.
     * All other screens will be assigned to group 0 (i.e., the main screen group) by default,
     *     which causes the virtual screen input method keyboard to malfunction (it disappears immediately after being clicked).
     * Refer to the following code to select the appropriate name for the virtual screen:
     * const std::set<std::string> INDIVIDUAL_SCREEN_GROUP_SET = {
     *   "CeliaView", "DevEcoViewer", "Cooperation-multi", "HwCast_AppModeDisplay"
     * };
     */
    const screenName = 'DevEcoViewer';
    const option: screen.VirtualScreenOption = {
      name: screenName,
      width: defaultDisplay.width,
      height: defaultDisplay.height,
      density: defaultDisplay.scaledDensity,
      surfaceId: '',
    };

    let created: screen.Screen;
    try {
      created = await screen.createVirtualScreen(option);
    } catch (e) {
      const err = e as BusinessError;
      throw new Error(`createVirtualScreen failed. Code:${err.code}, message:${err.message}`);
    }

    const runtime: VTSRuntimeInfo = {
      screenId: created.id,
      screenName,
      widthPx: option.width,
      heightPx: option.height,
      density: option.density,
    };

    const maybe = created as ScreenWithDisplayId;
    if (typeof maybe.displayId === 'number') {
      runtime.displayId = maybe.displayId;
    }

    try {
      await screen.makeUnique([runtime.screenId]);
    } catch (e) {
      const err = e as BusinessError;
      console.error(`${LOG_TAG} makeUnique failed. Code:${err.code}, message:${err.message}`);
    }

    this.runtime = runtime;
    this.imeRegistered = this.registerImeBestEffort(this.resolveDisplayId(runtime));
    this.applyImePolicyForTakeover();
    this.hasLaunchedApp = false;
    return runtime;
  }

  async destroy(): Promise<void> {
    if (!this.runtime) {
      return;
    }
    const screenId = this.runtime.screenId;
    const imeDisplayId = this.resolveDisplayId(this.runtime);
    this.runtime = null;
    this.lastTarget = null;
    this.hasLaunchedApp = false;
    if (this.imeRegistered) {
      this.unregisterImeBestEffort(imeDisplayId);
      this.imeRegistered = false;
    }
    try {
      await screen.destroyVirtualScreen(screenId);
    } catch (e) {
      const err = e as BusinessError;
      console.error(`${LOG_TAG} destroyVirtualScreen failed. Code:${err.code}, message:${err.message}`);
    }
  }

  async attachSurface(surfaceId: string): Promise<void> {
    const rt = this.requireRuntime();
    try {
      screen.setVirtualScreenSurface(rt.screenId, surfaceId);
    } catch (e) {
      const err = e as BusinessError;
      throw new Error(`setVirtualScreenSurface failed. Code:${err.code}, message:${err.message}`);
    }
  }

  async startApp(target: AppTarget): Promise<void> {
    const rt = this.requireRuntime();
    const want: Want = {
      bundleName: target.bundleName,
      abilityName: target.abilityName,
    };
    const opt: StartOptions = {
      displayId: this.resolveDisplayId(rt),
    };

    try {
      await this.startAbilityInvoker(want, opt);
      this.lastTarget = target;
      this.hasLaunchedApp = true;
    } catch (e) {
      const err = e as BusinessError;
      throw new Error(`startAbility failed. Code:${err.code}, message:${err.message}`);
    }
  }

  async inputText(text: string, sendEnter: boolean): Promise<void> {
    const rt = this.requireRuntime();
    void sendEnter;
    if (text.length > 0) {
      const imeReady = await this.waitForImeReady('inputText');
      if (!imeReady) {
        console.warn(`${LOG_TAG} inputText skipped: ime not ready`);
        return;
      }
      try {
        const ret = AppController.operateInputMethod(defaultInputMethodOperatorId.INSERT, text, 0, 0);
        void ret;
      } catch (e) {
        console.error(`${LOG_TAG} inputText insert failed: ${String(e)}`);
      }
    }
    // if (sendEnter) {
    //   AppController.operateInputMethod(defaultInputMethodOperatorId.ENTER, '', 0, 0);
    // }
  }

  async pressEnter(): Promise<void> {
    const rt = this.runtime;
    if (!rt) {
      console.warn(`${LOG_TAG} pressEnter skipped: runtime not ready`);
      return;
    }
    try {
      const imeReady = await this.waitForImeReady('pressEnter');
      if (!imeReady) {
        console.warn(`${LOG_TAG} pressEnter skipped: ime not ready`);
        return;
      }
      const ret = AppController.operateInputMethod(defaultInputMethodOperatorId.ENTER, '', 0, 0);
      void ret;
    } catch (e) {
      console.error(`${LOG_TAG} pressEnter failed: ${String(e)}`);
    }
  }

  async pressBackspace(): Promise<void> {
    const rt = this.runtime;
    if (!rt) {
      console.warn(`${LOG_TAG} pressBackspace skipped: runtime not ready`);
      return;
    }
    try {
      const imeReady = await this.waitForImeReady('pressBackspace');
      if (!imeReady) {
        console.warn(`${LOG_TAG} pressBackspace skipped: ime not ready`);
        return;
      }
      const ret = AppController.operateInputMethod(defaultInputMethodOperatorId.DELETE_FORWARD, '', 1, 0);
      void ret;
    } catch (e) {
      console.error(`${LOG_TAG} pressBackspace failed: ${String(e)}`);
    }
  }

  async pressKey(code: number | string): Promise<boolean> {
    const rt = this.runtime;
    if (!rt) {
      console.warn(`${LOG_TAG} pressKey skipped: runtime not ready`);
      return false;
    }

    const resolved = resolveCommonKeyCode(code);
    if (resolved === null) {
      console.warn(`${LOG_TAG} pressKey skipped: unsupported key code=${String(code)}`);
      return false;
    }

    try {
      await this.processSingleKeyEvent(resolved);
      return true;
    } catch (e) {
      console.error(`${LOG_TAG} pressKey failed: ${String(e)}`);
      return false;
    }
  }

  async pressComboKey(codes: Array<number | string>): Promise<boolean> {
    const rt = this.runtime;
    if (!rt) {
      console.warn(`${LOG_TAG} pressComboKey skipped: runtime not ready`);
      return false;
    }
    if (!Array.isArray(codes) || codes.length === 0) {
      console.warn(`${LOG_TAG} pressComboKey skipped: empty codes`);
      return false;
    }

    const resolved: Array<number> = [];
    for (const raw of codes) {
      const one = resolveCommonKeyCode(raw);
      if (one === null) {
        console.warn(`${LOG_TAG} pressComboKey skipped: unsupported key code=${String(raw)}`);
        return false;
      }
      if (!resolved.includes(one)) {
        resolved.push(one);
      }
    }
    if (resolved.length === 0) {
      console.warn(`${LOG_TAG} pressComboKey skipped: no valid codes`);
      return false;
    }

    try {
      await this.processComboKeyEvent(resolved);
      return true;
    } catch (e) {
      console.error(`${LOG_TAG} pressComboKey failed: ${String(e)}`);
      return false;
    }
  }

  async pressBack(): Promise<void> {
    await this.pressKey(KeyCode.KEYCODE_BACK);
  }

  async requestFocus(): Promise<boolean> {
    const rt = this.runtime;
    if (!rt) {
      console.warn(`${LOG_TAG} requestFocus skipped: runtime not ready`);
      return false;
    }

    const focusApi = AppController as AppControllerFocusApi;
    if (typeof focusApi.requestFocus !== 'function') {
      console.warn(`${LOG_TAG} requestFocus skipped: AppController.requestFocus not available`);
      return false;
    }

    const displayId = this.resolveDisplayId(rt);
    const target = this.lastTarget;
    let winLst: AppController.WindowInfoUnit[] = [];
    if (target) {
      winLst = this.listWindowsBestEffort(displayId, target.bundleName, target.abilityName);
      if (winLst.length === 0 && target.abilityName.length > 0) {
        winLst = this.listWindowsBestEffort(displayId, target.bundleName, '');
      }
    }
    if (winLst.length === 0) {
      winLst = this.listWindowsBestEffort(displayId, '', '');
    }
    if (
      winLst.length === 0 &&
      rt.displayId !== undefined &&
      rt.displayId !== rt.screenId
    ) {
      const fallbackDisplayId = rt.screenId;
      if (target) {
        winLst = this.listWindowsBestEffort(fallbackDisplayId, target.bundleName, target.abilityName);
        if (winLst.length === 0 && target.abilityName.length > 0) {
          winLst = this.listWindowsBestEffort(fallbackDisplayId, target.bundleName, '');
        }
      }
      if (winLst.length === 0) {
        winLst = this.listWindowsBestEffort(fallbackDisplayId, '', '');
      }
    }

    if (winLst.length === 0) {
      console.warn(`${LOG_TAG} requestFocus skipped: no window found`);
      return false;
    }

    const targetWindowId = winLst[0].windowId;
    const ok = focusApi.requestFocus(targetWindowId);
    console.info(`${LOG_TAG} requestFocus window ${targetWindowId} ok=${ok}`);
    return ok;
  }

  async screenshotPixelMap(): Promise<image.PixelMap | null> {
    const rt = this.requireRuntime();
    const imageSize: ScreenshotImageSize = { width: rt.widthPx, height: rt.heightPx };
    const displayId = this.resolveDisplayId(rt);
    const options: screenshot.ScreenshotOptions = {
      imageSize,
      displayId,
    };

    if (!this.hasLaunchedApp) {
      const wallpaper = await WallpaperState.getCurrentWallpaper();
      if (wallpaper) {
        return wallpaper;
      }
    }

    let imageMap: image.PixelMap | null = null;
    for (let i = 0; i < 3 && imageMap === null; i++) {
      try {
        imageMap = await screenshot.save(options);
      } catch (e) {
        const err = e as BusinessError;
        console.error(`${LOG_TAG} screenshot failed. Code:${err.code}, message:${err.message}`);
      }
    }

    return imageMap;
  }

  async screenshotBase64(): Promise<string> {
    const imageMap = await this.screenshotPixelMap();
    if (!imageMap) {
      return '';
    }
    return await pixelMapToBase64('image/jpeg', imageMap);
  }

  async screenshotLongPixelMap(): Promise<Array<image.PixelMap>> {
    const rt = this.requireRuntime();
    if (!this.hasLaunchedApp) {
      const one = await this.screenshotPixelMap();
      return one ? [one] : [];
    }

    const maxShots = LONG_SCREENSHOT_DEFAULT_SHOTS;
    const centerX = Math.max(0, Math.min(rt.widthPx - 1, Math.floor(rt.widthPx * 0.5)));

    const maxSwipeDistancePx = Math.max(1, Math.floor(rt.heightPx * (1 - LONG_SCREENSHOT_MIN_OVERLAP_RATIO)));
    let startY = Math.floor(rt.heightPx * LONG_SCREENSHOT_SWIPE_START_Y_RATIO);
    startY = Math.max(0, Math.min(rt.heightPx - 1, Math.max(startY, maxSwipeDistancePx)));
    const endY = Math.max(0, Math.min(rt.heightPx - 1, startY - maxSwipeDistancePx));
    if (startY === endY) {
      const one = await this.screenshotPixelMap();
      return one ? [one] : [];
    }

    const frames: Array<image.PixelMap> = [];
    let lastBlockHashes: Uint32Array | null = null;
    let stableCount = 0;
    for (let i = 0; i < maxShots; i += 1) {
      const pixelMap = await this.screenshotPixelMap();
      if (!pixelMap) {
        break;
      }
      let skipPush = false;
      try {
        const hashes = computePixelMapBlockHashes(pixelMap, LONG_SCREENSHOT_HASH_GRID_SIZE);
        if (lastBlockHashes) {
          const sameRatio = computeSameBlockRatio(lastBlockHashes, hashes);
          if (sameRatio >= LONG_SCREENSHOT_SAME_BLOCK_RATIO) {
            stableCount += 1;
          } else {
            stableCount = 0;
          }
          if (stableCount >= LONG_SCREENSHOT_STOP_ON_STABLE_COUNT) {
            skipPush = true;
          }
        }
        lastBlockHashes = hashes;
      } catch (e) {
        console.error(`${LOG_TAG} screenshotLongPixelMap signature failed: ${String(e)}`);
      }
      if (skipPush) {
        try {
          await pixelMap.release();
        } catch (_e) {
          // ignore
        }
        break;
      }
      frames.push(pixelMap);

      if (i >= maxShots - 1) {
        break;
      }
      try {
        await this.swipePx(centerX, startY, centerX, endY, LONG_SCREENSHOT_SWIPE_DURATION_MS);
      } catch (e) {
        console.error(`${LOG_TAG} screenshotLongPixelMap swipe failed: ${String(e)}`);
        break;
      }
      await sleep(LONG_SCREENSHOT_SETTLE_DELAY_MS);
    }

    return frames;
  }

  async screenshotLongBase64(): Promise<string> {
    const frames = await this.screenshotLongPixelMap();
    if (frames.length === 0) {
      return '';
    }
    const encoded: Array<string> = [];
    for (let i = 0; i < frames.length; i += 1) {
      const frame = frames[i];
      try {
        encoded.push(await pixelMapToBase64('image/jpeg', frame));
      } catch (e) {
        console.error(`${LOG_TAG} screenshotLongBase64 encode failed: ${String(e)}`);
        break;
      } finally {
        try {
          await frame.release();
        } catch (_e) {
          // ignore
        }
      }
    }
    return encoded.length > 0 ? JSON.stringify(encoded) : '';
  }

  async clickPx(x: number, y: number): Promise<void> {
    const rt = this.requireRuntime();
    const event: AppController.PointEvent = {
      id: -1,
      pressed: 0,
      action: POINTER_ACTION_DOWN,
      type: 2,
      displayX: x,
      displayY: y,
      displayId: rt.screenId,
    };
    AppController.processPointEvent(event);
    event.action = POINTER_ACTION_UP;
    AppController.processPointEvent(event);
  }

  async longClickPx(x: number, y: number, durationMs: number): Promise<void> {
    const rt = this.requireRuntime();
    const event: AppController.PointEvent = {
      id: -1,
      pressed: 0,
      action: POINTER_ACTION_DOWN,
      type: 2,
      displayX: x,
      displayY: y,
      displayId: rt.screenId,
    };
    AppController.processPointEvent(event);
    await sleep(Math.max(0, durationMs));
    event.action = POINTER_ACTION_UP;
    AppController.processPointEvent(event);
  }

  async swipePx(startX: number, startY: number, endX: number, endY: number, durationMs?: number): Promise<void> {
    const rt = this.requireRuntime();
    const event: AppController.PointEvent = {
      id: -1,
      pressed: 0,
      action: POINTER_ACTION_DOWN,
      type: 2,
      displayX: startX,
      displayY: startY,
      displayId: rt.screenId,
    };
    AppController.processPointEvent(event);
    event.action = POINTER_ACTION_MOVE;
    AppController.processPointEvent(event);

    const dx = endX - startX;
    const dy = endY - startY;
    let steps = Math.ceil(Math.max(Math.abs(dx), Math.abs(dy)) / 10);
    if (steps < 1) {
      steps = 1;
    }
    const stepDelay = durationMs !== undefined && durationMs > 0 ? Math.floor(durationMs / steps) : 0;
    const stepX = Math.floor(dx / steps);
    const stepY = Math.floor(dy / steps);
    let curX = startX;
    let curY = startY;
    for (let i = 0; i < steps; i++) {
      curX += stepX;
      curY += stepY;
      event.displayX = curX;
      event.displayY = curY;
      AppController.processPointEvent(event);
      if (stepDelay > 0) {
        await sleep(stepDelay);
      }
    }

    event.displayX = endX;
    event.displayY = endY;
    AppController.processPointEvent(event);

    event.action = POINTER_ACTION_UP;
    AppController.processPointEvent(event);
  }

  injectPointer(action: PointerAction, x: number, y: number, pointerId: number): void {
    const rt = this.requireRuntime();
    const safeId = Number.isFinite(pointerId) ? Math.max(0, Math.floor(pointerId)) : 0;
    const event: AppController.PointEvent = {
      id: safeId,
      pressed: 0,
      action: this.mapPointerAction(action),
      type: 2,
      displayX: x,
      displayY: y,
      displayId: rt.screenId,
    };
    AppController.processPointEvent(event);
  }

  async transferToMain(): Promise<number> {
    const rt = this.requireRuntime();
    const primaryFilterId = this.resolveDisplayId(rt);
    const target = this.lastTarget;
    let winLst: AppController.WindowInfoUnit[] = [];
    if (target) {
      winLst = this.listWindowsBestEffort(primaryFilterId, target.bundleName, target.abilityName);
      if (winLst.length === 0 && target.abilityName.length > 0) {
        winLst = this.listWindowsBestEffort(primaryFilterId, target.bundleName, '');
      }
      if (
        winLst.length === 0 &&
        rt.displayId !== undefined &&
        rt.displayId !== rt.screenId
      ) {
        winLst = this.listWindowsBestEffort(rt.screenId, target.bundleName, target.abilityName);
        if (winLst.length === 0 && target.abilityName.length > 0) {
          winLst = this.listWindowsBestEffort(rt.screenId, target.bundleName, '');
        }
      }
    }

    let transferred = 0;
    for (const winInfo of winLst) {
      try {
        const ret = AppController.transferWindowToTargetScreen(winInfo.windowId, 0, false);
        console.info(`${LOG_TAG} transferWindowToTargetScreen window ${winInfo.windowId} -> main ret ${ret}`);
        transferred += 1;
        const focusApi = AppController as AppControllerFocusApi;
        if (typeof focusApi.requestFocus === 'function') {
          const ok = focusApi.requestFocus(winInfo.windowId);
          console.info(`${LOG_TAG} requestFocus window ${winInfo.windowId} ok=${ok}`);
        }
      } catch (e) {
        const err = e as BusinessError;
        console.error(`${LOG_TAG} transferWindowToTargetScreen failed. Code:${err.code}, message:${err.message}`);
      }
    }

    if (target) {
      try {
        await this.startAppOnMain(target);
      } catch (e) {
        console.error(`${LOG_TAG} startAppOnMain failed: ${String(e)}`);
      }
    }
    this.hasLaunchedApp = false;
    return transferred;
  }

  private async processSingleKeyEvent(code: number): Promise<void> {
    const rt = this.requireRuntime();
    const displayId = this.resolveDisplayId(rt);
    const downTime = this.getActiveUptimeSeconds();
    const down: AppController.KeyEvent = {
      action: KEY_ACTION_DOWN,
      code,
      pressed: true,
      time: downTime,
      repeat: false,
      displayId,
    };
    AppController.processKeyEvent(down);

    const up: AppController.KeyEvent = {
      action: KEY_ACTION_UP,
      code: down.code,
      pressed: false,
      time: downTime,
      repeat: down.repeat,
      displayId: down.displayId,
    };
    AppController.processKeyEvent(up);
  }

  private async processComboKeyEvent(codes: Array<number>): Promise<void> {
    const rt = this.requireRuntime();
    const displayId = this.resolveDisplayId(rt);
    const time = this.getActiveUptimeSeconds();
    const primaryCode = codes[codes.length - 1];

    const downItems: Array<AppController.KeyItem> = codes.map((c: number) => this.makeKeyItem(c, true, time));
    const down: AppController.ComboKeyEvent = {
      action: KEY_ACTION_DOWN,
      code: primaryCode,
      repeat: false,
      displayId,
      keyItems: downItems,
    };
    AppController.processComboKeyEvent(down);

    const upItems: Array<AppController.KeyItem> = codes.map((c: number) => this.makeKeyItem(c, false, time));
    const up: AppController.ComboKeyEvent = {
      action: KEY_ACTION_UP,
      code: primaryCode,
      repeat: false,
      displayId,
      keyItems: upItems,
    };
    AppController.processComboKeyEvent(up);
  }

  private makeKeyItem(code: number, pressed: boolean, time: number): AppController.KeyItem {
    const item: AppController.KeyItem = { code, pressed, time };
    return item;
  }

  private requireRuntime(): VTSRuntimeInfo {
    if (!this.runtime) {
      throw new Error('VTS is not created');
    }
    return this.runtime;
  }

  private mapPointerAction(action: PointerAction): number {
    if (action === 'down') {
      return POINTER_ACTION_DOWN;
    }
    if (action === 'move') {
      return POINTER_ACTION_MOVE;
    }
    if (action === 'up') {
      return POINTER_ACTION_UP;
    }
    return POINTER_ACTION_CANCEL;
  }

  private isImeReady(): boolean {
    return this.imeRegistered && (lastImeStatus === 1 || lastCallingWindowId >= 0);
  }

  private async waitForImeReady(reason: string): Promise<boolean> {
    if (this.isImeReady()) {
      return true;
    }
    const ready = await this.pollImeReady(IME_READY_WAIT_MS);
    if (ready) {
      return true;
    }
    const rt = this.requireRuntime();
    const displayId = this.resolveDisplayId(rt);
    console.warn(
      `${LOG_TAG} ime wait timeout reason=${reason} status=${lastImeStatus} callingWindowId=${lastCallingWindowId} lastSignalMs=${lastImeSignalAtMs} displayId=${displayId}`
    );
    const reconnected = this.reconnectIme(displayId, reason);
    const readyAfter = await this.pollImeReady(IME_RECONNECT_WAIT_MS);
    if (!readyAfter) {
      console.warn(`${LOG_TAG} ime still not ready after reconnect reason=${reason} reconnected=${reconnected}`);
    }
    return readyAfter;
  }

  private async pollImeReady(timeoutMs: number): Promise<boolean> {
    const start = Date.now();
    while (Date.now() - start < timeoutMs) {
      if (this.isImeReady()) {
        return true;
      }
      await sleep(IME_POLL_INTERVAL_MS);
    }
    return this.isImeReady();
  }

  private reconnectIme(displayId: number, reason: string): boolean {
    if (this.userTakeoverActive) {
      if (this.imeRegistered) {
        this.unregisterImeBestEffort(displayId);
        this.imeRegistered = false;
      }
      console.info(`${LOG_TAG} ime reconnect skipped: user takeover active reason=${reason} displayId=${displayId}`);
      return false;
    }
    if (this.imeRegistered) {
      this.unregisterImeBestEffort(displayId);
      this.imeRegistered = false;
    }
    const ok = this.registerImeBestEffort(displayId);
    this.imeRegistered = ok;
    if (!ok) {
      console.warn(`${LOG_TAG} ime reconnect failed reason=${reason} displayId=${displayId}`);
    }
    return ok;
  }

  private applyImePolicyForTakeover(): void {
    const rt = this.runtime;
    if (!rt) {
      return;
    }
    const displayId = this.resolveDisplayId(rt);
    if (this.userTakeoverActive) {
      if (this.imeRegistered) {
        this.unregisterImeBestEffort(displayId);
        this.imeRegistered = false;
      }
      return;
    }
    if (!this.imeRegistered) {
      this.imeRegistered = this.registerImeBestEffort(displayId);
    }
  }

  // Calling this API will prevent the normal IME from popping up in the VTS.
  // However, if you don't call it, the IME input API won't work.
  // Note: The IME should be dynamically registered/unregistered when entering/leaving user takeover.
  private registerImeBestEffort(displayId: number): boolean {
    lastImeStatus = -1;
    lastCallingWindowId = -1;
    lastImeSignalAtMs = 0;
    const textCallback: AppController.inputViewCallback = {
      onSelectionChange: onSelectionChangeFunc,
      onTextChange: onTextChangeFunc,
      onFunctionKey: onFunctionKeyFunc,
    };
    const sysCallback: AppController.inputSysCallback = {
      onInputStatus: onInputStatusFunc,
      onSetCallingWindow: onSetCallingWindowFunc,
    };
    try {
      const ret = AppController.connectInputMethod(
        defaultInputMethodConnId.REGISTER_VIRTUALSCREEN,
        displayId,
        textCallback,
        sysCallback
      );
      if (ret !== 0) {
        console.error(`${LOG_TAG} connectInputMethod ret ${ret}`);
      }
      return ret === 0;
    } catch (e) {
      console.error(`${LOG_TAG} connectInputMethod failed: ${String(e)}`);
      return false;
    }
  }

  private unregisterImeBestEffort(displayId: number): void {
    const textCallback: AppController.inputViewCallback = {
      onSelectionChange: onSelectionChangeFunc,
      onTextChange: onTextChangeFunc,
      onFunctionKey: onFunctionKeyFunc,
    };
    const sysCallback: AppController.inputSysCallback = {
      onInputStatus: onInputStatusFunc,
      onSetCallingWindow: onSetCallingWindowFunc,
    };
    try {
      AppController.connectInputMethod(
        defaultInputMethodConnId.UNREGISTER_VIRTUALSCREEN,
        displayId,
        textCallback,
        sysCallback
      );
    } catch (e) {
      console.error(`${LOG_TAG} disconnectInputMethod failed: ${String(e)}`);
    }
  }

  private async startAppOnMain(target: AppTarget): Promise<void> {
    const want: Want = {
      bundleName: target.bundleName,
      abilityName: target.abilityName,
    };
    await this.startAbilityInvoker(want);
  }

  private listWindowsBestEffort(
    displayIdFilter: number,
    bundleNameFilter: string,
    abilityNameFilter: string
  ): AppController.WindowInfoUnit[] {
    const opt: AppController.ListWindowInfoOption = {
      displayIdFilter,
      windowInfoFilterOption: 1,
      windowInfoTypeOption: -1,
      windowIdFilter: 0,
      windowNameFilter: '',
      bundleNameFilter,
      abilityNameFilter,
    };

    try {
      return AppController.listWindowInfo(opt);
    } catch (e) {
      const err = e as BusinessError;
      console.error(`${LOG_TAG} listWindowInfo failed. Code:${err.code}, message:${err.message}`);
      return [];
    }
  }

  private resolveWindowIdBestEffort(windowId: number): number {
    const safe = Math.floor(windowId);
    if (Number.isFinite(safe) && safe > 0) {
      return safe;
    }
    try {
      const rt = this.runtime;
      const displayId = rt ? this.resolveDisplayId(rt) : undefined;
      if (typeof displayId === 'number') {
        return AppController.getFocusedWindowId(displayId);
      }
      return AppController.getFocusedWindowId();
    } catch (e) {
      console.warn(`${LOG_TAG} getFocusedWindowId failed: ${String(e)}`);
      return -1;
    }
  }

  private resolveDisplayId(rt: VTSRuntimeInfo): number {
    return rt.displayId ?? rt.screenId;
  }

  private getActiveUptimeSeconds(): number {
    return systemDateTime.getUptime(systemDateTime.TimeType.ACTIVE, true) / 1000;
  }
}

async function pixelMapToBase64(format: string, pixelMap: image.PixelMap): Promise<string> {
  const packer = image.createImagePacker();
  const value: ArrayBuffer = await packer.packing(pixelMap, {
    format,
    quality: 90,
    bufferSize: 50 * 1024 * 1024,
  });
  const base64 = new util.Base64Helper();
  return base64.encodeToStringSync(new Uint8Array(value));
}

function computePixelMapBlockHashes(pixelMap: image.PixelMap, gridSize: number): Uint32Array {
  const info = pixelMap.getImageInfoSync();
  const width = info.size.width;
  const height = info.size.height;
  const stride = pixelMap.getBytesNumberPerRow();

  const buffer = new ArrayBuffer(stride * height);
  const area: image.PositionArea = {
    pixels: buffer,
    offset: 0,
    stride,
    region: {
      x: 0,
      y: 0,
      size: { width, height },
    },
  };
  pixelMap.readPixelsSync(area);

  const bytes = new Uint8Array(buffer);
  const blocks = splitPixelMapBlocks(width, height, gridSize);
  return computeBlockHashes(bytes, stride, width, blocks.xStops, blocks.yStops);
}

interface PixelMapBlockSplit {
  xStops: Array<number>;
  yStops: Array<number>;
}

function splitPixelMapBlocks(
  width: number,
  height: number,
  gridSize: number
): PixelMapBlockSplit {
  const safeGrid = Math.max(1, Math.floor(gridSize));
  const xStops: Array<number> = new Array<number>(safeGrid + 1);
  const yStops: Array<number> = new Array<number>(safeGrid + 1);
  for (let i = 0; i <= safeGrid; i += 1) {
    xStops[i] = Math.floor((i * width) / safeGrid);
    yStops[i] = Math.floor((i * height) / safeGrid);
  }
  xStops[safeGrid] = width;
  yStops[safeGrid] = height;
  return { xStops, yStops };
}

function computeBlockHashes(
  bytes: Uint8Array,
  stride: number,
  width: number,
  xStops: Array<number>,
  yStops: Array<number>
): Uint32Array {
  const grid = Math.max(1, Math.min(xStops.length, yStops.length) - 1);
  const bytesPerPixel = Math.max(1, Math.floor(stride / Math.max(1, width)));
  const hashes = new Uint32Array(grid * grid);
  let idx = 0;
  for (let by = 0; by < grid; by += 1) {
    const yStart = yStops[by];
    const yEnd = yStops[by + 1];
    for (let bx = 0; bx < grid; bx += 1) {
      const xStart = xStops[bx];
      const xEnd = xStops[bx + 1];
      let hash = 2166136261;
      for (let y = yStart; y < yEnd; y += 1) {
        const rowOffset = y * stride;
        const start = Math.min(rowOffset + xStart * bytesPerPixel, rowOffset + stride);
        const end = Math.min(rowOffset + xEnd * bytesPerPixel, rowOffset + stride);
        for (let i = start; i < end; i += 1) {
          hash ^= bytes[i];
          hash = Math.imul(hash, 16777619);
        }
      }
      hashes[idx] = hash >>> 0;
      idx += 1;
    }
  }
  return hashes;
}

function computeSameBlockRatio(a: Uint32Array, b: Uint32Array): number {
  if (a.length === 0 || a.length !== b.length) {
    return 0;
  }
  let same = 0;
  for (let i = 0; i < a.length; i += 1) {
    if (a[i] === b[i]) {
      same += 1;
    }
  }
  return same / a.length;
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
