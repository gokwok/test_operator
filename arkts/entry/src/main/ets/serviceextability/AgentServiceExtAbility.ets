import ServiceExtensionAbility from '@ohos.app.ability.ServiceExtensionAbility';
import type Want from '@ohos.app.ability.Want';
import rpc from '@ohos.rpc';

import { AgentService } from '../features/agent_service/AgentService';
import { AgentServiceStub } from '../features/agent_service/ipc/AgentServiceStub';
import type { StartAbilityInvoker } from '../features/agentspace/VTSBackend';

export default class AgentServiceExtAbility extends ServiceExtensionAbility {
  private stub: AgentServiceStub | null = null;

  onCreate(_want: Want): void {
    const service = AgentService.getInstance();
    const startAbilityInvoker: StartAbilityInvoker = (want, options) => this.context.startAbility(want, options);
    service.init(this.context, startAbilityInvoker);
    this.stub = new AgentServiceStub(service);
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
}
