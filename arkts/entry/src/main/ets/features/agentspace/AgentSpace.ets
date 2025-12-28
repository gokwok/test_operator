import type { common } from '@kit.AbilityKit';

import { VTSBackend } from './VTSBackend';
import type { AppTarget, VTSRuntimeInfo } from './VTSBackend';

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

  async destroy(): Promise<void> {
    this.state = AgentSpaceState.STOPPING;
    try {
      await this.backend.destroy();
    } finally {
      this.state = AgentSpaceState.IDLE;
      this.viewerActive = false;
      this.surfaceId = '';
      this.surfaceAttached = false;
    }
  }
}
