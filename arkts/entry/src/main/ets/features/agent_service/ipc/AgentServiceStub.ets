import rpc from '@ohos.rpc';

import { AgentService } from '../AgentService';
import { AGENT_SERVICE_DESCRIPTOR, AgentServiceRequestCode } from './AgentServiceProtocol';

export class AgentServiceStub extends rpc.RemoteObject {
  private service: AgentService;

  constructor(service: AgentService) {
    super(AGENT_SERVICE_DESCRIPTOR);
    this.service = service;
  }

  onRemoteMessageRequest(
    code: number,
    data: rpc.MessageSequence,
    reply: rpc.MessageSequence,
    options: rpc.MessageOption
  ): boolean {
    void options;
    try {
      const token = data.readInterfaceToken();
      if (token !== AGENT_SERVICE_DESCRIPTOR) {
        reply.writeString('ERR_BAD_DESCRIPTOR');
        return false;
      }
      if (code === AgentServiceRequestCode.PING) {
        reply.writeString(this.service.ping());
        return true;
      }
      if (code === AgentServiceRequestCode.GET_SNAPSHOT) {
        reply.writeString(this.service.getSnapshotJson());
        return true;
      }
      reply.writeString('ERR_UNKNOWN_CODE');
      return false;
    } catch (e) {
      reply.writeString(`ERR_EXCEPTION:${String(e)}`);
      return false;
    }
  }
}
