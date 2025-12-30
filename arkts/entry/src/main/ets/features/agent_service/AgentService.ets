import type { common } from '@kit.AbilityKit';

import { AgentSpace } from '../agentspace/AgentSpace';
import type { AppTarget, StartAbilityInvoker } from '../agentspace/VTSBackend';
import { DeviceExecutor } from './DeviceExecutor';
import type { TaskEventBatch, TaskSnapshot } from './types';
import { TaskManager } from './TaskManager';

export class AgentService {
  private static instance: AgentService | null = null;
  private ctx: common.Context | null = null;
  private startAbilityInvoker: StartAbilityInvoker | null = null;
  private readonly createdAtMs: number = Date.now();
  private taskManager: TaskManager = new TaskManager();
  private deviceExecutor: DeviceExecutor | null = null;
  private lastScreenshotBase64: string = '';

  static getInstance(): AgentService {
    if (!AgentService.instance) {
      AgentService.instance = new AgentService();
    }
    return AgentService.instance;
  }

  init(ctx: common.Context, startAbilityInvoker: StartAbilityInvoker): void {
    this.ctx = ctx;
    this.startAbilityInvoker = startAbilityInvoker;
  }

  ping(): string {
    return `agent_service:${this.createdAtMs}`;
  }

  getSnapshotJson(): string {
    return JSON.stringify(this.taskManager.getSnapshot());
  }

  getContext(): common.Context | null {
    return this.ctx;
  }

  startFakeTask(sessionId: string): void {
    this.taskManager.startFakeTask(sessionId);
  }

  stopTask(reason: string): void {
    this.taskManager.stopTask(reason);
  }

  pullEventsJson(lastSequence: number): string {
    const batch: TaskEventBatch | null = this.taskManager.pullEvents(lastSequence);
    if (!batch) {
      return '';
    }
    return JSON.stringify(batch);
  }

  async ensureVtsJson(): Promise<string> {
    const runtime = await this.getDeviceExecutor().ensureVts();
    return JSON.stringify(runtime);
  }

  async getRuntimeJson(): Promise<string> {
    const runtime = await this.getDeviceExecutor().getRuntimeInfo();
    return JSON.stringify(runtime);
  }

  async launchApp(bundleName: string, abilityName: string): Promise<void> {
    const target: AppTarget = { bundleName, abilityName };
    await this.getDeviceExecutor().launchApp(target);
  }

  async clickNorm(xNorm: number, yNorm: number): Promise<void> {
    await this.getDeviceExecutor().clickNorm(xNorm, yNorm);
  }

  async captureScreenshotLength(): Promise<string> {
    const base64 = await this.getDeviceExecutor().screenshotBase64();
    this.lastScreenshotBase64 = base64;
    return String(base64.length);
  }

  readScreenshotChunk(offset: number, size: number): string {
    if (this.lastScreenshotBase64.length === 0) {
      return '';
    }
    const safeOffset = Math.max(0, Math.floor(offset));
    const safeSize = Math.max(0, Math.floor(size));
    if (safeSize === 0 || safeOffset >= this.lastScreenshotBase64.length) {
      return '';
    }
    const end = Math.min(this.lastScreenshotBase64.length, safeOffset + safeSize);
    return this.lastScreenshotBase64.substring(safeOffset, end);
  }

  async destroyVts(): Promise<void> {
    await this.getDeviceExecutor().destroy();
  }

  async stopVts(): Promise<number> {
    return await this.getDeviceExecutor().stop();
  }

  private ensureAgentSpace(): AgentSpace {
    const invoker = this.startAbilityInvoker;
    if (!invoker) {
      throw new Error('Service startAbility not initialized');
    }
    return AgentSpace.init(invoker);
  }

  private getDeviceExecutor(): DeviceExecutor {
    if (!this.deviceExecutor) {
      this.deviceExecutor = new DeviceExecutor(this.ensureAgentSpace());
    }
    return this.deviceExecutor;
  }
}
