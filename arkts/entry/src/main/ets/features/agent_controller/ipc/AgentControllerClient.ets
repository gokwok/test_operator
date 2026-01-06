import type { common, Want } from '@kit.AbilityKit';
import bundleManager from '@ohos.bundle.bundleManager';
import rpc from '@ohos.rpc';

import { AGENT_CONTROLLER_DESCRIPTOR, AgentControllerRequestCode } from './AgentControllerProtocol';
import type { TaskStartRequest, UserReply } from '../types';

interface DebugInteractPayload {
  content: string;
  options: Array<string>;
}

const SERVICE_BUNDLE_NAME: string = 'com.huawei.aios.agent.operation';
const SERVICE_ABILITY_NAME: string = 'AgentServiceExtAbility';

export class AgentControllerClient {
  private remote: rpc.IRemoteObject | null = null;
  private connectionId: number = -1;
  private connectPromise: Promise<void> | null = null;

  async connect(ctx: common.UIAbilityContext): Promise<void> {
    await this.connectInternal(ctx);
  }

  async connectFromExtension(ctx: common.UIExtensionContext): Promise<void> {
    await this.connectInternal(ctx);
  }

  private async connectInternal(ctx: common.UIAbilityContext | common.UIExtensionContext): Promise<void> {
    if (this.remote) {
      return;
    }
    if (this.connectPromise) {
      return this.connectPromise;
    }
    const promise = new Promise<void>((resolve: () => void, reject: (reason: Error) => void) => {
      const want: Want = {
        bundleName: SERVICE_BUNDLE_NAME,
        abilityName: SERVICE_ABILITY_NAME,
      };
      const options: common.ConnectOptions = {
        onConnect: (_elementName: bundleManager.ElementName, remote: rpc.IRemoteObject) => {
          this.remote = remote;
          resolve();
        },
        onDisconnect: (_elementName: bundleManager.ElementName) => {
          this.remote = null;
        },
        onFailed: (code: number) => {
          this.remote = null;
          reject(new Error(`Service connect failed: ${code}`));
        }
      };
      try {
        this.connectionId = ctx.connectServiceExtensionAbility(want, options);
      } catch (e) {
        reject(new Error(`Service connect failed: ${String(e)}`));
      }
    });
    this.connectPromise = promise;
    return promise.then(() => {
      this.connectPromise = null;
    }).catch((err: Error) => {
      this.connectPromise = null;
      throw err;
    });
  }

  async disconnect(ctx: common.UIAbilityContext): Promise<void> {
    await this.disconnectInternal(ctx);
  }

  async disconnectFromExtension(ctx: common.UIExtensionContext): Promise<void> {
    await this.disconnectInternal(ctx);
  }

  private async disconnectInternal(ctx: common.UIAbilityContext | common.UIExtensionContext): Promise<void> {
    if (this.connectionId === -1) {
      return;
    }
    const id = this.connectionId;
    this.connectionId = -1;
    this.remote = null;
    await ctx.disconnectServiceExtensionAbility(id);
  }

  async ping(): Promise<string> {
    return await this.sendRequest(AgentControllerRequestCode.PING);
  }

  async getSnapshot(): Promise<string> {
    return await this.sendRequest(AgentControllerRequestCode.GET_SNAPSHOT);
  }

  async startTask(request: TaskStartRequest): Promise<void> {
    const payload = JSON.stringify(request);
    await this.sendRequestWithString(AgentControllerRequestCode.START_TASK, payload);
  }

  async stopTask(reason: string): Promise<void> {
    await this.sendRequestWithString(AgentControllerRequestCode.STOP_TASK, reason);
  }

  async pullEvents(lastSequence: number): Promise<string> {
    return await this.sendRequestWithInt(AgentControllerRequestCode.PULL_EVENTS, lastSequence);
  }

  async submitUserReply(reply: UserReply): Promise<void> {
    const payload = JSON.stringify(reply);
    await this.sendRequestWithString(AgentControllerRequestCode.SUBMIT_USER_REPLY, payload);
  }

  async startTaskText(taskText: string): Promise<void> {
    await this.sendRequestWithString(AgentControllerRequestCode.START_TASK_TEXT, taskText);
  }

  async ensureVts(): Promise<string> {
    return await this.sendRequest(AgentControllerRequestCode.ENSURE_VTS);
  }

  async getRuntime(): Promise<string> {
    return await this.sendRequest(AgentControllerRequestCode.GET_RUNTIME);
  }

  async launchApp(bundleName: string, abilityName: string): Promise<void> {
    await this.sendRequestWithTwoStrings(AgentControllerRequestCode.LAUNCH_APP, bundleName, abilityName);
  }

  async clickNorm(xNorm: number, yNorm: number): Promise<void> {
    await this.sendRequestWithTwoInts(AgentControllerRequestCode.CLICK_NORM, xNorm, yNorm);
  }

  async screenshot(): Promise<string> {
    const lengthText = await this.sendRequest(AgentControllerRequestCode.SCREENSHOT);
    const total = parseInt(lengthText, 10);
    if (!Number.isFinite(total) || total <= 0) {
      return '';
    }
    const chunkSize = 16384;
    let offset = 0;
    const parts: Array<string> = [];
    while (offset < total) {
      const size = Math.min(chunkSize, total - offset);
      const chunk = await this.sendRequestWithTwoInts(AgentControllerRequestCode.SCREENSHOT_CHUNK, offset, size);
      if (chunk.length === 0) {
        break;
      }
      parts.push(chunk);
      offset += chunk.length;
    }
    return parts.join('');
  }

  async destroyVts(): Promise<void> {
    await this.sendRequest(AgentControllerRequestCode.DESTROY_VTS);
  }

  async stopVts(): Promise<string> {
    return await this.sendRequest(AgentControllerRequestCode.STOP_VTS);
  }

  async setViewerActive(active: boolean): Promise<void> {
    await this.sendRequestWithInt(AgentControllerRequestCode.SET_VIEWER_ACTIVE, active ? 1 : 0);
  }

  async getViewerMode(): Promise<string> {
    return await this.sendRequest(AgentControllerRequestCode.GET_VIEWER_MODE);
  }

  async setViewerMode(mode: string): Promise<void> {
    await this.sendRequestWithString(AgentControllerRequestCode.SET_VIEWER_MODE, mode);
  }

  async attachViewerSurface(surfaceId: string): Promise<void> {
    await this.sendRequestWithString(AgentControllerRequestCode.ATTACH_VIEWER_SURFACE, surfaceId);
  }

  async sendViewerEvent(action: string, xNorm: number, yNorm: number, pointerId: number): Promise<void> {
    await this.sendRequestWithStringAndInts(
      AgentControllerRequestCode.SEND_VIEWER_EVENT,
      action,
      xNorm,
      yNorm,
      pointerId
    );
  }

  async pullViewerEffects(lastSequence: number): Promise<string> {
    return await this.sendRequestWithInt(AgentControllerRequestCode.PULL_VIEWER_EFFECTS, lastSequence);
  }

  async transferToMain(): Promise<string> {
    return await this.sendRequest(AgentControllerRequestCode.TRANSFER_TO_MAIN);
  }

  async transferToMainAndReleaseIfFinished(): Promise<void> {
    await this.sendRequest(AgentControllerRequestCode.TRANSFER_AND_RELEASE_IF_FINISHED);
  }

  async releaseAgentSpaceIfFinished(): Promise<boolean> {
    const result = await this.sendRequest(AgentControllerRequestCode.RELEASE_AGENT_SPACE_IF_FINISHED);
    return result === '1';
  }

  async closeLiveViewNow(): Promise<void> {
    await this.sendRequest(AgentControllerRequestCode.CLOSE_LIVE_VIEW);
  }

  async actionDoubleClick(xNorm: number, yNorm: number): Promise<string> {
    return await this.sendRequestWithTwoInts(AgentControllerRequestCode.ACTION_DOUBLE_CLICK, xNorm, yNorm);
  }

  async actionLongClick(xNorm: number, yNorm: number, durationMs: number): Promise<string> {
    return await this.sendRequestWithThreeInts(AgentControllerRequestCode.ACTION_LONG_CLICK, xNorm, yNorm, durationMs);
  }

  async actionSwipe(
    startX: number,
    startY: number,
    endX: number,
    endY: number,
    durationMs: number
  ): Promise<string> {
    return await this.sendRequestWithFiveInts(
      AgentControllerRequestCode.ACTION_SWIPE,
      startX,
      startY,
      endX,
      endY,
      durationMs
    );
  }

  async actionType(content: string, hasPoint: boolean, xNorm: number, yNorm: number): Promise<string> {
    const flag = hasPoint ? 1 : 0;
    return await this.sendRequestWithStringAndInts(AgentControllerRequestCode.ACTION_TYPE, content, flag, xNorm, yNorm);
  }

  async actionPressBack(): Promise<string> {
    return await this.sendRequest(AgentControllerRequestCode.ACTION_PRESS_BACK);
  }

  async actionPressEnter(): Promise<string> {
    return await this.sendRequest(AgentControllerRequestCode.ACTION_PRESS_ENTER);
  }

  async setCloudEndpoint(endpoint: string): Promise<void> {
    await this.sendRequestWithString(AgentControllerRequestCode.SET_CLOUD_ENDPOINT, endpoint);
  }

  async requestTaskOver(content: string): Promise<void> {
    await this.sendRequestWithString(AgentControllerRequestCode.REQUEST_TASK_OVER, content);
  }

  async requestSupplement(content: string): Promise<void> {
    await this.sendRequestWithString(AgentControllerRequestCode.REQUEST_SUPPLEMENT, content);
  }

  async submitUserNote(content: string): Promise<void> {
    await this.sendRequestWithString(AgentControllerRequestCode.SUBMIT_USER_NOTE, content);
  }

  async debugCallUser(content: string): Promise<void> {
    await this.sendRequestWithString(AgentControllerRequestCode.DEBUG_CALL_USER, content);
  }

  async debugTakeOver(content: string): Promise<void> {
    await this.sendRequestWithString(AgentControllerRequestCode.DEBUG_TAKE_OVER, content);
  }

  async debugInteract(content: string, options: Array<string>): Promise<void> {
    const payload: DebugInteractPayload = { content, options };
    const text = JSON.stringify(payload);
    await this.sendRequestWithString(AgentControllerRequestCode.DEBUG_INTERACT, text);
  }

  private async sendRequest(code: number): Promise<string> {
    const remote = this.remote;
    if (!remote) {
      throw new Error('Service not connected');
    }
    const data = rpc.MessageSequence.create();
    const reply = rpc.MessageSequence.create();
    const option = new rpc.MessageOption();
    try {
      data.writeInterfaceToken(AGENT_CONTROLLER_DESCRIPTOR);
      const result = await remote.sendMessageRequest(code, data, reply, option);
      const response = reply.readString();
      if (result.errCode !== 0) {
        throw new Error(`IPC error: ${result.errCode}`);
      }
      return response;
    } finally {
      data.reclaim();
      reply.reclaim();
    }
  }

  private async sendRequestWithString(code: number, value: string): Promise<string> {
    const remote = this.remote;
    if (!remote) {
      throw new Error('Service not connected');
    }
    const data = rpc.MessageSequence.create();
    const reply = rpc.MessageSequence.create();
    const option = new rpc.MessageOption();
    try {
      data.writeInterfaceToken(AGENT_CONTROLLER_DESCRIPTOR);
      data.writeString(value);
      const result = await remote.sendMessageRequest(code, data, reply, option);
      const response = reply.readString();
      if (result.errCode !== 0) {
        throw new Error(`IPC error: ${result.errCode}`);
      }
      return response;
    } finally {
      data.reclaim();
      reply.reclaim();
    }
  }

  private async sendRequestWithInt(code: number, value: number): Promise<string> {
    const remote = this.remote;
    if (!remote) {
      throw new Error('Service not connected');
    }
    const data = rpc.MessageSequence.create();
    const reply = rpc.MessageSequence.create();
    const option = new rpc.MessageOption();
    try {
      data.writeInterfaceToken(AGENT_CONTROLLER_DESCRIPTOR);
      data.writeInt(value);
      const result = await remote.sendMessageRequest(code, data, reply, option);
      const response = reply.readString();
      if (result.errCode !== 0) {
        throw new Error(`IPC error: ${result.errCode}`);
      }
      return response;
    } finally {
      data.reclaim();
      reply.reclaim();
    }
  }

  private async sendRequestWithTwoStrings(code: number, first: string, second: string): Promise<string> {
    const remote = this.remote;
    if (!remote) {
      throw new Error('Service not connected');
    }
    const data = rpc.MessageSequence.create();
    const reply = rpc.MessageSequence.create();
    const option = new rpc.MessageOption();
    try {
      data.writeInterfaceToken(AGENT_CONTROLLER_DESCRIPTOR);
      data.writeString(first);
      data.writeString(second);
      const result = await remote.sendMessageRequest(code, data, reply, option);
      const response = reply.readString();
      if (result.errCode !== 0) {
        throw new Error(`IPC error: ${result.errCode}`);
      }
      return response;
    } finally {
      data.reclaim();
      reply.reclaim();
    }
  }

  private async sendRequestWithTwoInts(code: number, first: number, second: number): Promise<string> {
    const remote = this.remote;
    if (!remote) {
      throw new Error('Service not connected');
    }
    const data = rpc.MessageSequence.create();
    const reply = rpc.MessageSequence.create();
    const option = new rpc.MessageOption();
    try {
      data.writeInterfaceToken(AGENT_CONTROLLER_DESCRIPTOR);
      data.writeInt(first);
      data.writeInt(second);
      const result = await remote.sendMessageRequest(code, data, reply, option);
      const response = reply.readString();
      if (result.errCode !== 0) {
        throw new Error(`IPC error: ${result.errCode}`);
      }
      return response;
    } finally {
      data.reclaim();
      reply.reclaim();
    }
  }

  private async sendRequestWithThreeInts(code: number, first: number, second: number, third: number): Promise<string> {
    const remote = this.remote;
    if (!remote) {
      throw new Error('Service not connected');
    }
    const data = rpc.MessageSequence.create();
    const reply = rpc.MessageSequence.create();
    const option = new rpc.MessageOption();
    try {
      data.writeInterfaceToken(AGENT_CONTROLLER_DESCRIPTOR);
      data.writeInt(first);
      data.writeInt(second);
      data.writeInt(third);
      const result = await remote.sendMessageRequest(code, data, reply, option);
      const response = reply.readString();
      if (result.errCode !== 0) {
        throw new Error(`IPC error: ${result.errCode}`);
      }
      return response;
    } finally {
      data.reclaim();
      reply.reclaim();
    }
  }

  private async sendRequestWithFiveInts(
    code: number,
    first: number,
    second: number,
    third: number,
    fourth: number,
    fifth: number
  ): Promise<string> {
    const remote = this.remote;
    if (!remote) {
      throw new Error('Service not connected');
    }
    const data = rpc.MessageSequence.create();
    const reply = rpc.MessageSequence.create();
    const option = new rpc.MessageOption();
    try {
      data.writeInterfaceToken(AGENT_CONTROLLER_DESCRIPTOR);
      data.writeInt(first);
      data.writeInt(second);
      data.writeInt(third);
      data.writeInt(fourth);
      data.writeInt(fifth);
      const result = await remote.sendMessageRequest(code, data, reply, option);
      const response = reply.readString();
      if (result.errCode !== 0) {
        throw new Error(`IPC error: ${result.errCode}`);
      }
      return response;
    } finally {
      data.reclaim();
      reply.reclaim();
    }
  }

  private async sendRequestWithStringAndInts(
    code: number,
    text: string,
    first: number,
    second: number,
    third: number
  ): Promise<string> {
    const remote = this.remote;
    if (!remote) {
      throw new Error('Service not connected');
    }
    const data = rpc.MessageSequence.create();
    const reply = rpc.MessageSequence.create();
    const option = new rpc.MessageOption();
    try {
      data.writeInterfaceToken(AGENT_CONTROLLER_DESCRIPTOR);
      data.writeString(text);
      data.writeInt(first);
      data.writeInt(second);
      data.writeInt(third);
      const result = await remote.sendMessageRequest(code, data, reply, option);
      const response = reply.readString();
      if (result.errCode !== 0) {
        throw new Error(`IPC error: ${result.errCode}`);
      }
      return response;
    } finally {
      data.reclaim();
      reply.reclaim();
    }
  }
}
