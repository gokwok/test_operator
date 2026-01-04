import ServiceExtensionAbility from '@ohos.app.ability.ServiceExtensionAbility';
import type Want from '@ohos.app.ability.Want';
import rpc from '@ohos.rpc';
import { hilog } from '@kit.PerformanceAnalysisKit';

import { AgentService } from '../features/agent_service/AgentService';
import { AgentServiceStub } from '../features/agent_service/ipc/AgentServiceStub';
import type { StartAbilityInvoker } from '../features/agentspace/VTSBackend';

const LOG_TAG: string = '[AgentServiceExt]';
const DOMAIN: number = 0x0000;
const ACTION_START_TASK: string = 'start_task';

interface AgentServiceStartParams {
  action?: string;
  taskText?: string;
}

export default class AgentServiceExtAbility extends ServiceExtensionAbility {
  private stub: AgentServiceStub | null = null;

  onCreate(_want: Want): void {
    const service = AgentService.getInstance();
    const startAbilityInvoker: StartAbilityInvoker = (want, options) => this.context.startAbility(want, options);
    service.init(this.context, startAbilityInvoker);
    this.stub = new AgentServiceStub(service);
  }

  async onRequest(want: Want, _startId: number): Promise<void> {
    const params = want.parameters as AgentServiceStartParams;
    const action = typeof want.action === 'string' && want.action.length > 0
      ? want.action
      : (typeof params?.action === 'string' ? params.action : '');
    if (action !== ACTION_START_TASK) {
      if (action === 'keep_alive') {
        hilog.info(DOMAIN, LOG_TAG, 'keep_alive received');
      }
      return;
    }
    const taskText = this.resolveTaskText(want, params);
    if (taskText.length === 0) {
      hilog.warn(DOMAIN, LOG_TAG, 'start_task ignored: empty taskText');
      return;
    }

    try {
      const service = AgentService.getInstance();
      await service.startTaskWithDefaults(taskText);
    } catch (e) {
      hilog.error(DOMAIN, LOG_TAG, 'start_task failed: %{public}s', String(e));
    }
  }

  onConnect(_want: Want): rpc.RemoteObject {
    if (!this.stub) {
      this.stub = new AgentServiceStub(AgentService.getInstance());
    }
    return this.stub;
  }

  onDisconnect(_want: Want): void {
    void _want;
  }

  onDestroy(): void {
    AgentService.getInstance().releaseEfficiencyResourcesOnExit();
    this.stub = null;
  }

  private resolveTaskText(want: Want, params: AgentServiceStartParams): string {
    if (typeof want.uri === 'string' && want.uri.length > 0) {
      try {
        const decoded = decodeURIComponent(want.uri);
        const trimmed = decoded.trim();
        if (trimmed.length > 0) {
          return trimmed;
        }
      } catch (_e) {
        const fallback = want.uri.trim();
        if (fallback.length > 0) {
          return fallback;
        }
      }
    }
    const raw = typeof params?.taskText === 'string' ? params.taskText : '';
    return raw.trim();
  }
}
