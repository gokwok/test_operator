import rpc from '@ohos.rpc';

import { AgentService } from '../AgentService';
import { AGENT_SERVICE_DESCRIPTOR, AgentServiceRequestCode } from './AgentServiceProtocol';

export class AgentServiceStub extends rpc.RemoteObject {
  private service: AgentService;

  constructor(service: AgentService) {
    super(AGENT_SERVICE_DESCRIPTOR);
    this.service = service;
  }

  async onRemoteMessageRequest(
    code: number,
    data: rpc.MessageSequence,
    reply: rpc.MessageSequence,
    options: rpc.MessageOption
  ): Promise<boolean> {
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
      if (code === AgentServiceRequestCode.ENSURE_VTS) {
        const payload = await this.service.ensureVtsJson();
        reply.writeString(payload);
        return true;
      }
      if (code === AgentServiceRequestCode.GET_RUNTIME) {
        const payload = await this.service.getRuntimeJson();
        reply.writeString(payload);
        return true;
      }
      if (code === AgentServiceRequestCode.LAUNCH_APP) {
        const bundleName = data.readString();
        const abilityName = data.readString();
        await this.service.launchApp(bundleName, abilityName);
        reply.writeString('ok');
        return true;
      }
      if (code === AgentServiceRequestCode.CLICK_NORM) {
        const xNorm = data.readInt();
        const yNorm = data.readInt();
        await this.service.clickNorm(xNorm, yNorm);
        reply.writeString('ok');
        return true;
      }
      if (code === AgentServiceRequestCode.SCREENSHOT) {
        const payload = await this.service.screenshotBase64();
        reply.writeString(payload);
        return true;
      }
      if (code === AgentServiceRequestCode.DESTROY_VTS) {
        await this.service.destroyVts();
        reply.writeString('ok');
        return true;
      }
      if (code === AgentServiceRequestCode.STOP_VTS) {
        const count = await this.service.stopVts();
        reply.writeString(String(count));
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
