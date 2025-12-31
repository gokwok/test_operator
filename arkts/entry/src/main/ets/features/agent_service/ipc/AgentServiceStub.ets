import rpc from '@ohos.rpc';

import { AGENT_SERVICE_DESCRIPTOR, AgentServiceRequestCode } from './AgentServiceProtocol';
import { AgentService } from '../AgentService';
import type { AgentSpacePointerAction, AgentSpacePointerEvent, AgentSpaceViewerMode } from '../../agentspace/AgentSpace';
import type { TaskStartRequest, UserReply } from '../types';

interface DebugInteractPayload {
  content?: string;
  options?: Array<string>;
}

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
      if (code === AgentServiceRequestCode.START_TASK) {
        const payload = data.readString();
        const request = JSON.parse(payload) as TaskStartRequest;
        this.service.startTask(request);
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
      if (code === AgentServiceRequestCode.SUBMIT_USER_REPLY) {
        const payload = data.readString();
        const replyPayload = JSON.parse(payload) as UserReply;
        this.service.submitUserReply(replyPayload);
        reply.writeString('ok');
        return true;
      }
      if (code === AgentServiceRequestCode.START_TASK_TEXT) {
        const taskText = data.readString();
        await this.service.startTaskWithDefaults(taskText);
        reply.writeString('ok');
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
      if (code === AgentServiceRequestCode.TRANSFER_TO_MAIN) {
        const count = await this.service.transferToMain();
        reply.writeString(String(count));
        return true;
      }
      if (code === AgentServiceRequestCode.ACTION_DOUBLE_CLICK) {
        const xNorm = data.readInt();
        const yNorm = data.readInt();
        const result = await this.service.actionDoubleClick(xNorm, yNorm);
        reply.writeString(result);
        return true;
      }
      if (code === AgentServiceRequestCode.ACTION_LONG_CLICK) {
        const xNorm = data.readInt();
        const yNorm = data.readInt();
        const durationMs = data.readInt();
        const result = await this.service.actionLongClick(xNorm, yNorm, durationMs);
        reply.writeString(result);
        return true;
      }
      if (code === AgentServiceRequestCode.ACTION_SWIPE) {
        const startX = data.readInt();
        const startY = data.readInt();
        const endX = data.readInt();
        const endY = data.readInt();
        const durationMs = data.readInt();
        const result = await this.service.actionSwipe(startX, startY, endX, endY, durationMs);
        reply.writeString(result);
        return true;
      }
      if (code === AgentServiceRequestCode.ACTION_TYPE) {
        const content = data.readString();
        const hasPoint = data.readInt() === 1;
        const xNorm = data.readInt();
        const yNorm = data.readInt();
        const result = await this.service.actionType(content, hasPoint, xNorm, yNorm);
        reply.writeString(result);
        return true;
      }
      if (code === AgentServiceRequestCode.ACTION_PRESS_BACK) {
        const result = await this.service.actionPressBack();
        reply.writeString(result);
        return true;
      }
      if (code === AgentServiceRequestCode.ACTION_PRESS_ENTER) {
        const result = await this.service.actionPressEnter();
        reply.writeString(result);
        return true;
      }
      if (code === AgentServiceRequestCode.SET_CLOUD_ENDPOINT) {
        const endpoint = data.readString();
        this.service.setCloudEndpoint(endpoint);
        reply.writeString('ok');
        return true;
      }
      if (code === AgentServiceRequestCode.DEBUG_CALL_USER) {
        const content = data.readString();
        this.service.debugCallUser(content);
        reply.writeString('ok');
        return true;
      }
      if (code === AgentServiceRequestCode.DEBUG_TAKE_OVER) {
        const content = data.readString();
        this.service.debugTakeOver(content);
        reply.writeString('ok');
        return true;
      }
      if (code === AgentServiceRequestCode.DEBUG_INTERACT) {
        const payload = data.readString();
        const parsed = JSON.parse(payload) as DebugInteractPayload;
        const content = parsed?.content ?? '';
        const options = Array.isArray(parsed?.options) ? parsed.options : [];
        this.service.debugInteract(content, options);
        reply.writeString('ok');
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
