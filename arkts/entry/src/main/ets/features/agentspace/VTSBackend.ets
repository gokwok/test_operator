import type { common, StartOptions, Want } from '@kit.AbilityKit';
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

export interface AppTarget {
  bundleName: string;
  abilityName: string;
}

export interface VTSRuntimeInfo {
  screenId: number;
  displayId?: number;
  widthPx: number;
  heightPx: number;
  density: number;
  screenName: string;
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

export class VTSBackend {
  private ctx: common.UIAbilityContext;
  private runtime: VTSRuntimeInfo | null = null;
  private lastTarget: AppTarget | null = null;

  constructor(ctx: common.UIAbilityContext) {
    this.ctx = ctx;
  }

  updateContext(ctx: common.UIAbilityContext): void {
    this.ctx = ctx;
  }

  getRuntime(): VTSRuntimeInfo | null {
    return this.runtime;
  }

  async create(): Promise<VTSRuntimeInfo> {
    if (this.runtime) {
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
    return runtime;
  }

  async destroy(): Promise<void> {
    if (!this.runtime) {
      return;
    }
    const screenId = this.runtime.screenId;
    this.runtime = null;
    this.lastTarget = null;
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
      displayId: rt.displayId ?? rt.screenId,
    };

    try {
      await this.ctx.startAbility(want, opt);
      this.lastTarget = target;
    } catch (e) {
      const err = e as BusinessError;
      throw new Error(`startAbility failed. Code:${err.code}, message:${err.message}`);
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

  async swipePx(startX: number, startY: number, endX: number, endY: number): Promise<void> {
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
    }

    event.displayX = endX;
    event.displayY = endY;
    AppController.processPointEvent(event);

    event.action = POINTER_ACTION_UP;
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

  private async startAppOnMain(target: AppTarget): Promise<void> {
    const want: Want = {
      bundleName: target.bundleName,
      abilityName: target.abilityName,
    };
    await this.ctx.startAbility(want);
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
