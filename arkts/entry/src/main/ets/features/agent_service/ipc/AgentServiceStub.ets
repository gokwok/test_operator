import rpc from '@ohos.rpc';

import { AgentService } from '../AgentService';
import type { AgentSpacePointerAction, AgentSpacePointerEvent, AgentSpaceViewerMode } from '../../agentspace/AgentSpace';
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
        const lengthText = await this.service.captureScreenshotLength();
        reply.writeString(lengthText);
        return true;
      }
      if (code === AgentServiceRequestCode.SCREENSHOT_CHUNK) {
        const offset = data.readInt();
        const size = data.readInt();
        const chunk = this.service.readScreenshotChunk(offset, size);
        reply.writeString(chunk);
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
      if (code === AgentServiceRequestCode.SET_VIEWER_ACTIVE) {
        const active = data.readInt() === 1;
        await this.service.setViewerActive(active);
        reply.writeString('ok');
        return true;
      }
      if (code === AgentServiceRequestCode.GET_VIEWER_MODE) {
        const mode = await this.service.getViewerMode();
        reply.writeString(mode);
        return true;
      }
      if (code === AgentServiceRequestCode.SET_VIEWER_MODE) {
        const rawMode = data.readString();
        const mode = this.normalizeViewerMode(rawMode);
        await this.service.setViewerMode(mode);
        reply.writeString('ok');
        return true;
      }
      if (code === AgentServiceRequestCode.ATTACH_VIEWER_SURFACE) {
        const surfaceId = data.readString();
        await this.service.attachViewerSurface(surfaceId);
        reply.writeString('ok');
        return true;
      }
      if (code === AgentServiceRequestCode.SEND_VIEWER_EVENT) {
        const actionText = data.readString();
        const action = this.normalizePointerAction(actionText);
        if (!action) {
          reply.writeString('ERR_BAD_ACTION');
          return false;
        }
        const xNormInt = data.readInt();
        const yNormInt = data.readInt();
        const pointerId = data.readInt();
        const event: AgentSpacePointerEvent = {
          action,
          xNorm: this.normFromInt(xNormInt),
          yNorm: this.normFromInt(yNormInt),
          pointerId,
        };
        await this.service.sendViewerEvent(event);
        reply.writeString('ok');
        return true;
      }
      if (code === AgentServiceRequestCode.PULL_VIEWER_EFFECTS) {
        const lastSequence = data.readInt();
        const payload = await this.service.pullViewerEffectsJson(lastSequence);
        reply.writeString(payload);
        return true;
      }
      reply.writeString('ERR_UNKNOWN_CODE');
      return false;
    } catch (e) {
      reply.writeString(`ERR_EXCEPTION:${String(e)}`);
      return false;
    }
  }

  private normalizeViewerMode(mode: string): AgentSpaceViewerMode {
    if (mode === 'control') {
      return 'control';
    }
    return 'display';
  }

  private normalizePointerAction(action: string): AgentSpacePointerAction | null {
    if (action === 'down' || action === 'move' || action === 'up' || action === 'cancel') {
      return action;
    }
    return null;
  }

  private normFromInt(value: number): number {
    if (!Number.isFinite(value)) {
      return 0;
    }
    const clamped = Math.min(Math.max(value, 0), 1000);
    return clamped / 1000;
  }
}
