import ServiceExtensionAbility from '@ohos.app.ability.ServiceExtensionAbility';
import type Want from '@ohos.app.ability.Want';
import rpc from '@ohos.rpc';
import { hilog } from '@kit.PerformanceAnalysisKit';

import { AgentService } from '../features/agent_service/AgentService';
import { AgentServiceStub } from '../features/agent_service/ipc/AgentServiceStub';
import type { StartAbilityInvoker } from '../features/agentspace/VTSBackend';
import type { AppTargetDescriptor, TaskStartRequest } from '../features/agent_service/types';
import { AppState } from '../features/state/app_state';
import { AgentSettings } from '../features/settings/AgentSettings';

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
      return;
    }
    const taskText = this.resolveTaskText(want, params);
    if (taskText.length === 0) {
      hilog.warn(DOMAIN, LOG_TAG, 'start_task ignored: empty taskText');
      return;
    }

    try {
      const settings = await AgentSettings.load(this.context);
      const installedApps = await this.safeListAppNames();
      const appTargets = await this.safeListAppTargets();
      const request: TaskStartRequest = {
        sessionId: '',
        taskText,
        installedApps,
        appTargets,
        waitMs: settings.waitMs,
      };
      const service = AgentService.getInstance();
      service.applySettings(settings);
      if (settings.cloudEndpoint.length > 0) {
        service.setCloudEndpoint(settings.cloudEndpoint);
      }
      service.startTask(request);
      await service.hideFloatWindow();
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
    this.stub = null;
  }

  private async safeListAppNames(): Promise<string[]> {
    try {
      return await AppState.listAppNames(this.context);
    } catch (e) {
      hilog.warn(DOMAIN, LOG_TAG, 'listAppNames failed: %{public}s', String(e));
      return [];
    }
  }

  private async safeListAppTargets(): Promise<Array<AppTargetDescriptor>> {
    try {
      return await AppState.listAppTargets(this.context);
    } catch (e) {
      hilog.warn(DOMAIN, LOG_TAG, 'listAppTargets failed: %{public}s', String(e));
      return [];
    }
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
