import type { common, Want } from '@kit.AbilityKit';
import bundleManager from '@ohos.bundle.bundleManager';
import rpc from '@ohos.rpc';

import { AGENT_SERVICE_DESCRIPTOR, AgentServiceRequestCode } from './AgentServiceProtocol';

const SERVICE_BUNDLE_NAME: string = 'com.huawei.aios.agent.operation';
const SERVICE_ABILITY_NAME: string = 'AgentServiceExtAbility';

export class AgentServiceClient {
  private remote: rpc.IRemoteObject | null = null;
  private connectionId: number = -1;
  private connectPromise: Promise<void> | null = null;

  async connect(ctx: common.UIAbilityContext): Promise<void> {
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
    if (this.connectionId === -1) {
      return;
    }
    const id = this.connectionId;
    this.connectionId = -1;
    this.remote = null;
    await ctx.disconnectServiceExtensionAbility(id);
  }

  async ping(): Promise<string> {
    return await this.sendRequest(AgentServiceRequestCode.PING);
  }

  async getSnapshot(): Promise<string> {
    return await this.sendRequest(AgentServiceRequestCode.GET_SNAPSHOT);
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
      data.writeInterfaceToken(AGENT_SERVICE_DESCRIPTOR);
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
