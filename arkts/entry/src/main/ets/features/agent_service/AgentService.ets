import type { common } from '@kit.AbilityKit';

import type { TaskSnapshot } from './types';

export class AgentService {
  private static instance: AgentService | null = null;
  private ctx: common.Context | null = null;
  private readonly createdAtMs: number = Date.now();
  private snapshot: TaskSnapshot = {
    sessionId: '',
    status: 'idle',
    stepIndex: 0,
    lastDescription: '',
    lastActionName: '',
    lastResultStatus: '',
    lastError: '',
  };

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
    return JSON.stringify(this.snapshot);
  }

  getContext(): common.Context | null {
    return this.ctx;
  }
}
