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
      if (code === AgentServiceRequestCode.START_FAKE_TASK) {
        const sessionId = data.readString();
        this.service.startFakeTask(sessionId);
        reply.writeString('ok');
        return true;
      }
      if (code === AgentServiceRequestCode.STOP_TASK) {
        const reason = data.readString();
        this.service.stopTask(reason);
        reply.writeString('ok');
        return true;
      }
      if (code === AgentServiceRequestCode.PULL_EVENTS) {
        const lastSequence = data.readInt();
        reply.writeString(this.service.pullEventsJson(lastSequence));
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
