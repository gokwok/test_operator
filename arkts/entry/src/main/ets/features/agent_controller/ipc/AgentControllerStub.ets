import rpc from '@ohos.rpc';

import { AGENT_CONTROLLER_DESCRIPTOR, AgentControllerRequestCode } from './AgentControllerProtocol';
import { AgentController } from '../AgentController';
import type { AgentSpacePointerAction, AgentSpacePointerEvent, AgentSpaceViewerMode } from '../agentspace/AgentSpace';
import type { TaskStartRequest, UserReply } from '../types';

interface DebugInteractPayload {
  content?: string;
  options?: Array<string>;
}

export class AgentControllerStub extends rpc.RemoteObject {
  private service: AgentController;

  constructor(service: AgentController) {
    super(AGENT_CONTROLLER_DESCRIPTOR);
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
      if (token !== AGENT_CONTROLLER_DESCRIPTOR) {
        reply.writeString('ERR_BAD_DESCRIPTOR');
        return false;
      }
      if (code === AgentControllerRequestCode.PING) {
        reply.writeString(this.service.ping());
        return true;
      }
      if (code === AgentControllerRequestCode.GET_SNAPSHOT) {
        reply.writeString(this.service.getSnapshotJson());
        return true;
      }
      if (code === AgentControllerRequestCode.START_TASK) {
        const payload = data.readString();
        const request = JSON.parse(payload) as TaskStartRequest;
        await this.service.startTask(request);
        reply.writeString('ok');
        return true;
      }
      if (code === AgentControllerRequestCode.STOP_TASK) {
        const reason = data.readString();
        this.service.stopTask(reason);
        reply.writeString('ok');
        return true;
      }
      if (code === AgentControllerRequestCode.PULL_EVENTS) {
        const lastSequence = data.readInt();
        reply.writeString(this.service.pullEventsJson(lastSequence));
        return true;
      }
      if (code === AgentControllerRequestCode.SUBMIT_USER_REPLY) {
        const payload = data.readString();
        const replyPayload = JSON.parse(payload) as UserReply;
        this.service.submitUserReply(replyPayload);
        reply.writeString('ok');
        return true;
      }
      if (code === AgentControllerRequestCode.START_TASK_TEXT) {
        const taskText = data.readString();
        await this.service.startTaskWithDefaults(taskText);
        reply.writeString('ok');
        return true;
      }
      if (code === AgentControllerRequestCode.ENSURE_VTS) {
        const payload = await this.service.ensureVtsJson();
        reply.writeString(payload);
        return true;
      }
      if (code === AgentControllerRequestCode.GET_RUNTIME) {
        const payload = await this.service.getRuntimeJson();
        reply.writeString(payload);
        return true;
      }
      if (code === AgentControllerRequestCode.LAUNCH_APP) {
        const bundleName = data.readString();
        const abilityName = data.readString();
        await this.service.launchApp(bundleName, abilityName);
        reply.writeString('ok');
        return true;
      }
      if (code === AgentControllerRequestCode.CLICK_NORM) {
        const xNorm = data.readInt();
        const yNorm = data.readInt();
        await this.service.clickNorm(xNorm, yNorm);
        reply.writeString('ok');
        return true;
      }
      if (code === AgentControllerRequestCode.SCREENSHOT) {
        const lengthText = await this.service.captureScreenshotLength();
        reply.writeString(lengthText);
        return true;
      }
      if (code === AgentControllerRequestCode.SCREENSHOT_CHUNK) {
        const offset = data.readInt();
        const size = data.readInt();
        const chunk = this.service.readScreenshotChunk(offset, size);
        reply.writeString(chunk);
        return true;
      }
      if (code === AgentControllerRequestCode.DESTROY_VTS) {
        await this.service.destroyVts();
        reply.writeString('ok');
        return true;
      }
      if (code === AgentControllerRequestCode.STOP_VTS) {
        const count = await this.service.stopVts();
        reply.writeString(String(count));
        return true;
      }
      if (code === AgentControllerRequestCode.SET_VIEWER_ACTIVE) {
        const active = data.readInt() === 1;
        await this.service.setViewerActive(active);
        reply.writeString('ok');
        return true;
      }
      if (code === AgentControllerRequestCode.GET_VIEWER_MODE) {
        const mode = await this.service.getViewerMode();
        reply.writeString(mode);
        return true;
      }
      if (code === AgentControllerRequestCode.SET_VIEWER_MODE) {
        const rawMode = data.readString();
        const mode = this.normalizeViewerMode(rawMode);
        await this.service.setViewerMode(mode);
        reply.writeString('ok');
        return true;
      }
      if (code === AgentControllerRequestCode.ATTACH_VIEWER_SURFACE) {
        const surfaceId = data.readString();
        await this.service.attachViewerSurface(surfaceId);
        reply.writeString('ok');
        return true;
      }
      if (code === AgentControllerRequestCode.SEND_VIEWER_EVENT) {
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
      if (code === AgentControllerRequestCode.PULL_VIEWER_EFFECTS) {
        const lastSequence = data.readInt();
        const payload = await this.service.pullViewerEffectsJson(lastSequence);
        reply.writeString(payload);
        return true;
      }
      if (code === AgentControllerRequestCode.TRANSFER_TO_MAIN) {
        const count = await this.service.transferToMain();
        reply.writeString(String(count));
        return true;
      }
      if (code === AgentControllerRequestCode.TRANSFER_AND_RELEASE_IF_FINISHED) {
        const count = await this.service.transferToMainAndReleaseIfFinished();
        reply.writeString(String(count));
        return true;
      }
      if (code === AgentControllerRequestCode.RELEASE_AGENT_SPACE_IF_FINISHED) {
        const released = await this.service.releaseAgentSpaceIfFinished();
        reply.writeString(released ? '1' : '0');
        return true;
      }
      if (code === AgentControllerRequestCode.CLOSE_LIVE_VIEW) {
        await this.service.closeLiveViewNow();
        reply.writeString('ok');
        return true;
      }
      if (code === AgentControllerRequestCode.ACTION_DOUBLE_CLICK) {
        const xNorm = data.readInt();
        const yNorm = data.readInt();
        const result = await this.service.actionDoubleClick(xNorm, yNorm);
        reply.writeString(result);
        return true;
      }
      if (code === AgentControllerRequestCode.ACTION_LONG_CLICK) {
        const xNorm = data.readInt();
        const yNorm = data.readInt();
        const durationMs = data.readInt();
        const result = await this.service.actionLongClick(xNorm, yNorm, durationMs);
        reply.writeString(result);
        return true;
      }
      if (code === AgentControllerRequestCode.ACTION_SWIPE) {
        const startX = data.readInt();
        const startY = data.readInt();
        const endX = data.readInt();
        const endY = data.readInt();
        const durationMs = data.readInt();
        const result = await this.service.actionSwipe(startX, startY, endX, endY, durationMs);
        reply.writeString(result);
        return true;
      }
      if (code === AgentControllerRequestCode.ACTION_TYPE) {
        const content = data.readString();
        const hasPoint = data.readInt() === 1;
        const xNorm = data.readInt();
        const yNorm = data.readInt();
        const result = await this.service.actionType(content, hasPoint, xNorm, yNorm);
        reply.writeString(result);
        return true;
      }
      if (code === AgentControllerRequestCode.ACTION_PRESS_BACK) {
        const result = await this.service.actionPressBack();
        reply.writeString(result);
        return true;
      }
      if (code === AgentControllerRequestCode.ACTION_PRESS_ENTER) {
        const result = await this.service.actionPressEnter();
        reply.writeString(result);
        return true;
      }
      if (code === AgentControllerRequestCode.SET_CLOUD_ENDPOINT) {
        const endpoint = data.readString();
        this.service.setCloudEndpoint(endpoint);
        reply.writeString('ok');
        return true;
      }
      if (code === AgentControllerRequestCode.REQUEST_TASK_OVER) {
        const content = data.readString();
        this.service.requestTaskOver(content);
        reply.writeString('ok');
        return true;
      }
      if (code === AgentControllerRequestCode.REQUEST_SUPPLEMENT) {
        const content = data.readString();
        this.service.requestSupplement(content);
        reply.writeString('ok');
        return true;
      }
      if (code === AgentControllerRequestCode.SUBMIT_USER_NOTE) {
        const content = data.readString();
        this.service.submitUserNote(content);
        reply.writeString('ok');
        return true;
      }
      if (code === AgentControllerRequestCode.DEBUG_CALL_USER) {
        const content = data.readString();
        this.service.debugCallUser(content);
        reply.writeString('ok');
        return true;
      }
      if (code === AgentControllerRequestCode.DEBUG_TAKE_OVER) {
        const content = data.readString();
        this.service.debugTakeOver(content);
        reply.writeString('ok');
        return true;
      }
      if (code === AgentControllerRequestCode.DEBUG_INTERACT) {
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
