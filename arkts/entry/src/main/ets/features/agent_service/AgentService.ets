import type { common } from '@kit.AbilityKit';

import type { TaskEventBatch, TaskSnapshot } from './types';
import { TaskManager } from './TaskManager';

export class AgentService {
  private static instance: AgentService | null = null;
  private ctx: common.Context | null = null;
  private readonly createdAtMs: number = Date.now();
  private taskManager: TaskManager = new TaskManager();

  static getInstance(): AgentService {
    if (!AgentService.instance) {
      AgentService.instance = new AgentService();
    }
    return AgentService.instance;
  }

  init(ctx: common.Context): void {
    this.ctx = ctx;
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
}
