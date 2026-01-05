import type { StartOptions, Want } from '@kit.AbilityKit';
import { display, screen, screenshot } from '@kit.ArkUI';
import type { BusinessError } from '@kit.BasicServicesKit';
import image from '@ohos.multimedia.image';
import util from '@ohos.util';
import * as AppController from '@hms.ai.appController';

const LOG_TAG: string = '[VTSBackend]';

const POINTER_ACTION_UNKNOWN: number = 0;
const POINTER_ACTION_CANCEL: number = 1;
const POINTER_ACTION_DOWN: number = 2;
const POINTER_ACTION_MOVE: number = 3;
const POINTER_ACTION_UP: number = 4;

export type VTSBackendPointerAction = 'down' | 'move' | 'up' | 'cancel';
type PointerAction = VTSBackendPointerAction;

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

function safeStringify(value: AppController.onSelChgParam): string {
  try {
    return JSON.stringify(value);
  } catch (e) {
    return String(value);
  }
}

function formatTextSample(value: string, limit: number): string {
  if (value.length === 0) {
    return '';
  }
  const sanitized = value.replace(/\r/g, '\\r').replace(/\n/g, '\\n');
  if (sanitized.length <= limit) {
    return sanitized;
  }
  return `${sanitized.slice(0, limit)}...`;
}

function onInputStatusFunc(unused: undefined, status: number): void {
  void unused;
  console.info(`${LOG_TAG} ime status=${status}`);
}

function onSetCallingWindowFunc(unused: undefined, windowId: number): void {
  void unused;
  console.info(`${LOG_TAG} ime callingWindowId=${windowId}`);
}

function onSelectionChangeFunc(unused: undefined, data: AppController.onSelChgParam): void {
  void unused;
  console.info(`${LOG_TAG} ime selectionChange=${safeStringify(data)}`);
}

function onTextChangeFunc(unused: undefined, data: string): void {
  void unused;
  const text = typeof data === 'string' ? data : '';
  const sample = formatTextSample(text, 64);
  console.info(`${LOG_TAG} ime textChange len=${text.length} sample=${sample}`);
}

function onFunctionKeyFunc(unused: undefined, funcKey: number): void {
  void unused;
  console.info(`${LOG_TAG} ime functionKey=${funcKey}`);
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
  create(): Promise<VTSRuntimeInfo>;
  attachSurface(surfaceId: string): Promise<void>;
  startApp(target: AppTarget): Promise<void>;
  clickPx(x: number, y: number): Promise<void>;
  longClickPx(x: number, y: number, durationMs: number): Promise<void>;
  swipePx(startX: number, startY: number, endX: number, endY: number, durationMs?: number): Promise<void>;
  inputText(text: string, sendEnter: boolean): Promise<void>;
  pressEnter(): Promise<void>;
  screenshotBase64(): Promise<string>;
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

  constructor(startAbilityInvoker: StartAbilityInvoker) {
    this.startAbilityInvoker = startAbilityInvoker;
  }

  updateStartAbility(invoker: StartAbilityInvoker): void {
    this.startAbilityInvoker = invoker;
  }

  getRuntime(): VTSRuntimeInfo | null {
    return this.runtime;
  }

  async create(): Promise<VTSRuntimeInfo> {
    if (this.runtime) {
      if (!this.imeRegistered) {
        console.info(`${LOG_TAG} ime not registered, register for displayId=${this.runtime.displayId ?? this.runtime.screenId}`);
        this.imeRegistered = this.registerImeBestEffort(this.runtime.displayId ?? this.runtime.screenId);
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

    const screenName = `agent.vts.${Date.now()}`;
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
    console.info(
      `${LOG_TAG} runtime created screenId=${runtime.screenId} displayId=${runtime.displayId ?? runtime.screenId} ` +
      `size=${runtime.widthPx}x${runtime.heightPx} density=${runtime.density}`
    );
    this.imeRegistered = this.registerImeBestEffort(runtime.displayId ?? runtime.screenId);
    console.info(`${LOG_TAG} ime registered=${this.imeRegistered}`);
    return runtime;
  }

  async destroy(): Promise<void> {
    if (!this.runtime) {
      return;
    }
    const screenId = this.runtime.screenId;
    const imeDisplayId = this.runtime.displayId ?? this.runtime.screenId;
    this.runtime = null;
    this.lastTarget = null;
    if (this.imeRegistered) {
      console.info(`${LOG_TAG} unregister ime displayId=${imeDisplayId}`);
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
      console.info(`${LOG_TAG} attachSurface surfaceId=${surfaceId} screenId=${rt.screenId}`);
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
      displayId: rt.displayId ?? rt.screenId,
    };

    try {
      console.info(`${LOG_TAG} startApp bundle=${target.bundleName} ability=${target.abilityName} displayId=${opt.displayId}`);
      await this.startAbilityInvoker(want, opt);
      this.lastTarget = target;
    } catch (e) {
      const err = e as BusinessError;
      throw new Error(`startAbility failed. Code:${err.code}, message:${err.message}`);
    }
  }

  async inputText(text: string, sendEnter: boolean): Promise<void> {
    const rt = this.requireRuntime();
    const displayId = rt.displayId ?? rt.screenId;
    const sample = formatTextSample(text, 64);
    console.info(
      `${LOG_TAG} inputText start len=${text.length} sendEnter=${sendEnter} displayId=${displayId} screenId=${rt.screenId} sample=${sample}`
    );
    if (text.length > 0) {
      try {
        const ret = AppController.operateInputMethod(defaultInputMethodOperatorId.INSERT, text, 0, 0);
        console.info(`${LOG_TAG} inputText insert ret=${String(ret)} len=${text.length}`);
      } catch (e) {
        console.error(`${LOG_TAG} inputText insert failed: ${String(e)}`);
      }
    } else {
      console.info(`${LOG_TAG} inputText skip insert: empty text`);
    }
    if (sendEnter) {
      console.info(`${LOG_TAG} inputText sendEnter requested (enter sent via pressEnter)`);
    }
    // if (sendEnter) {
    //   AppController.operateInputMethod(defaultInputMethodOperatorId.ENTER, '', 0, 0);
    // }
  }

  async pressEnter(): Promise<void> {
    if (!this.runtime) {
      console.warn(`${LOG_TAG} pressEnter skipped: runtime not ready`);
      return;
    }
    try {
      const ret = AppController.operateInputMethod(defaultInputMethodOperatorId.ENTER, '', 0, 0);
      console.info(`${LOG_TAG} pressEnter ret=${String(ret)}`);
    } catch (e) {
      console.error(`${LOG_TAG} pressEnter failed: ${String(e)}`);
    }
  }

  async screenshotBase64(): Promise<string> {
    const rt = this.requireRuntime();
    const imageSize: ScreenshotImageSize = { width: rt.widthPx, height: rt.heightPx };
    const displayId = rt.displayId ?? rt.screenId;
    const options: screenshot.ScreenshotOptions = {
      imageSize,
      displayId,
    };

    let imageMap: image.PixelMap | null = null;
    for (let i = 0; i < 3 && imageMap === null; i++) {
      try {
        imageMap = await screenshot.save(options);
      } catch (e) {
        const err = e as BusinessError;
        console.error(`${LOG_TAG} screenshot failed. Code:${err.code}, message:${err.message}`);
      }
    }

    if (imageMap === null) {
      return '';
    }

    return await pixelMapToBase64('image/jpeg', imageMap);
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
    const primaryFilterId = rt.displayId ?? rt.screenId;
    let winLst = this.listWindowsBestEffort(primaryFilterId);
    if (
      winLst.length === 0 &&
      rt.displayId !== undefined &&
      rt.displayId !== rt.screenId
    ) {
      winLst = this.listWindowsBestEffort(rt.screenId);
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

    if (this.lastTarget) {
      try {
        await this.startAppOnMain(this.lastTarget);
      } catch (e) {
        console.error(`${LOG_TAG} startAppOnMain failed: ${String(e)}`);
      }
    }
    return transferred;
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


  private registerImeBestEffort(displayId: number): boolean {
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
      console.info(`${LOG_TAG} connectInputMethod register displayId=${displayId}`);
      const ret = AppController.connectInputMethod(
        defaultInputMethodConnId.REGISTER_VIRTUALSCREEN,
        displayId,
        textCallback,
        sysCallback
      );
      if (ret !== 0) {
        console.error(`${LOG_TAG} connectInputMethod ret ${ret}`);
      }
      console.info(`${LOG_TAG} connectInputMethod register ret=${ret}`);
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
      console.info(`${LOG_TAG} connectInputMethod unregister displayId=${displayId}`);
      AppController.connectInputMethod(
        defaultInputMethodConnId.UNREGISTER_VIRTUALSCREEN,
        displayId,
        textCallback,
        sysCallback
      );
      console.info(`${LOG_TAG} connectInputMethod unregister ok`);
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

  private listWindowsBestEffort(displayIdFilter: number): AppController.WindowInfoUnit[] {
    const opt: AppController.ListWindowInfoOption = {
      displayIdFilter,
      windowInfoFilterOption: 1,
      windowInfoTypeOption: -1,
      windowIdFilter: 0,
      windowNameFilter: '',
      bundleNameFilter: '',
      abilityNameFilter: '',
    };

    try {
      return AppController.listWindowInfo(opt);
    } catch (e) {
      const err = e as BusinessError;
      console.error(`${LOG_TAG} listWindowInfo failed. Code:${err.code}, message:${err.message}`);
      return [];
    }
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

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
