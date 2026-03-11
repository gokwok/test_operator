import type { common, Want } from '@kit.AbilityKit';
import bundleManager from '@ohos.bundle.bundleManager';
import rpc from '@ohos.rpc';

const AI_DISPATCH_BUNDLE_NAME: string = 'com.huawei.hmos.aidispatchservice';
const AI_DISPATCH_MODULE_NAME: string = 'entry';
const AI_DISPATCH_ABILITY_NAME: string = 'AiDispatchRemoteAbility';
const AI_DISPATCH_SERVICE_NAME: string = 'AiDispatch';
const AI_DISPATCH_MESSAGE_NAME_EXECUTE_INTENT: string = 'ExecuteIntent';
const AI_DISPATCH_MESSAGE_VERSION: string = '1.0.0';
const AI_DISPATCH_INTENT_REQUEST_VERSION: string = '1.0.0';
const AI_DISPATCH_BUSINESS_NAME: string = 'scenemanager';
const AI_DISPATCH_BASE_SERVICE_DESCRIPTOR: string = 'idl.IBaseSdkService';
const AI_DISPATCH_CALLBACK_DESCRIPTOR: string = 'idl.IServiceCallback';
const AI_DISPATCH_PROCESS_MESSAGE_CODE: number = 0;
const AI_DISPATCH_CALLBACK_RESULT_CODE: number = 0;
const AI_DISPATCH_CALLBACK_NOTIFY_CODE: number = 1;
const AI_DISPATCH_STRING_SLICE_SIZE: number = 10000;

export const AI_DISPATCH_NOTE_TARGET_BUNDLE_NAME: string = 'com.huawei.hmos.notepad';
export const AI_DISPATCH_SEARCH_EXECUTE_MODE: string = 'background';

interface AiDispatchSearchIntentParam {
  query: string;
}

interface AiDispatchExecuteParam {
  bundleName: string;
  intentName: string;
  executeMode: string;
  intentRequestVersion: string;
  intentParam: AiDispatchSearchIntentParam;
}

interface AiDispatchPayload {
  executeParam: AiDispatchExecuteParam;
}

interface AiDispatchContentDataItem {
  payload: AiDispatchPayload;
}

interface AiDispatchContent {
  contentData: Array<AiDispatchContentDataItem>;
}

interface AiDispatchSession {
  serviceName: string;
  messageName: string;
  senderName: string;
  traceId: string;
  messageVersion: string;
  businessName: string;
}

interface AiDispatchRequestPayload {
  session: AiDispatchSession;
  content: AiDispatchContent;
}

interface AiDispatchCallbackPayload {
  resultCode: number;
  message: string;
}

export interface AiDispatchSearchRequest {
  senderBundleName: string;
  targetBundleName: string;
  intentName: string;
  query: string;
}

export interface AiDispatchSearchResult {
  callbackCode: number;
  callbackMessage: string;
  notifyMessages: Array<string>;
  parsedMessage: Object | null;
}

class AiDispatchServiceCallback extends rpc.RemoteObject {
  private readonly resolveResult: (payload: AiDispatchCallbackPayload) => void;
  private readonly notifyMessages: Array<string>;

  constructor(
    resolveResult: (payload: AiDispatchCallbackPayload) => void,
    notifyMessages: Array<string>
  ) {
    super(AI_DISPATCH_CALLBACK_DESCRIPTOR);
    this.resolveResult = resolveResult;
    this.notifyMessages = notifyMessages;
  }

  onRemoteMessageRequest(
    code: number,
    data: rpc.MessageSequence,
    reply: rpc.MessageSequence,
    options: rpc.MessageOption
  ): boolean {
    void options;
    const token = data.readInterfaceToken();
    if (token !== AI_DISPATCH_CALLBACK_DESCRIPTOR) {
      reply.writeNoException();
      return false;
    }
    if (code === AI_DISPATCH_CALLBACK_RESULT_CODE) {
      const resultCode = data.readInt();
      const message = readSlicedString(data);
      const payload: AiDispatchCallbackPayload = {
        resultCode,
        message,
      };
      this.resolveResult(payload);
      reply.writeNoException();
      return true;
    }
    if (code === AI_DISPATCH_CALLBACK_NOTIFY_CODE) {
      this.notifyMessages.push(readSlicedString(data));
      reply.writeNoException();
      return true;
    }
    reply.writeNoException();
    return false;
  }
}

function writeSlicedString(sequence: rpc.MessageSequence, value: string): void {
  const raw = value;
  if (raw.length === 0) {
    sequence.writeInt(0);
    return;
  }
  const segmentCount = Math.ceil(raw.length / AI_DISPATCH_STRING_SLICE_SIZE);
  sequence.writeInt(segmentCount);
  let index = 0;
  while (index < raw.length) {
    const nextIndex = Math.min(index + AI_DISPATCH_STRING_SLICE_SIZE, raw.length);
    sequence.writeString(raw.slice(index, nextIndex));
    index = nextIndex;
  }
}

function readSlicedString(sequence: rpc.MessageSequence): string {
  const segmentCount = sequence.readInt();
  if (segmentCount <= 0) {
    return '';
  }
  const parts: Array<string> = [];
  let index = 0;
  while (index < segmentCount) {
    parts.push(sequence.readString());
    index += 1;
  }
  return parts.join('');
}

export class AiDispatchSearchClient {
  private remote: rpc.IRemoteObject | null = null;
  private connectionId: number = -1;
  private connectPromise: Promise<void> | null = null;

  async executeSearchIntent(
    ctx: common.UIAbilityContext,
    request: AiDispatchSearchRequest
  ): Promise<AiDispatchSearchResult> {
    await this.connect(ctx);
    const dispatchPayload = this.buildRequestPayload(request);
    const dispatchPayloadText = JSON.stringify(dispatchPayload);
    const notifyMessages: Array<string> = [];
    let settled = false;
    let resolvePromise: ((payload: AiDispatchCallbackPayload) => void) | null = null;
    const callbackPromise = new Promise<AiDispatchCallbackPayload>((resolve) => {
      resolvePromise = resolve;
    });
    const callback = new AiDispatchServiceCallback((payload: AiDispatchCallbackPayload) => {
      if (settled) {
        return;
      }
      settled = true;
      const resolver = resolvePromise;
      if (resolver) {
        resolver(payload);
      }
    }, notifyMessages);

    try {
      await this.sendProcessMessage(dispatchPayloadText, callback);
    } catch (error) {
      settled = true;
      throw new Error(`AiDispatch IPC failed: ${String(error)}`);
    }

    const callbackPayload = await callbackPromise;
    const parsedMessage = this.parseJsonObject(callbackPayload.message);
    const result: AiDispatchSearchResult = {
      callbackCode: callbackPayload.resultCode,
      callbackMessage: callbackPayload.message,
      notifyMessages,
      parsedMessage,
    };
    return result;
  }

  private async connect(ctx: common.UIAbilityContext): Promise<void> {
    if (this.remote) {
      return;
    }
    if (this.connectPromise) {
      return this.connectPromise;
    }
    const promise = new Promise<void>((resolve: () => void, reject: (reason: Error) => void) => {
      const want: Want = {
        bundleName: AI_DISPATCH_BUNDLE_NAME,
        moduleName: AI_DISPATCH_MODULE_NAME,
        abilityName: AI_DISPATCH_ABILITY_NAME,
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
          reject(new Error(`AiDispatch connect failed: ${code}`));
        }
      };
      try {
        this.connectionId = ctx.connectServiceExtensionAbility(want, options);
      } catch (error) {
        reject(new Error(`AiDispatch connect failed: ${String(error)}`));
      }
    });
    this.connectPromise = promise;
    return promise.then(() => {
      this.connectPromise = null;
    }).catch((error: Error) => {
      this.connectPromise = null;
      throw error;
    });
  }

  private buildRequestPayload(request: AiDispatchSearchRequest): AiDispatchRequestPayload {
    const intentParam: AiDispatchSearchIntentParam = {
      query: request.query,
    };
    const executeParam: AiDispatchExecuteParam = {
      bundleName: request.targetBundleName,
      intentName: request.intentName,
      executeMode: AI_DISPATCH_SEARCH_EXECUTE_MODE,
      intentRequestVersion: AI_DISPATCH_INTENT_REQUEST_VERSION,
      intentParam,
    };
    const payload: AiDispatchPayload = {
      executeParam,
    };
    const contentDataItem: AiDispatchContentDataItem = {
      payload,
    };
    const content: AiDispatchContent = {
      contentData: [contentDataItem],
    };
    const session: AiDispatchSession = {
      serviceName: AI_DISPATCH_SERVICE_NAME,
      messageName: AI_DISPATCH_MESSAGE_NAME_EXECUTE_INTENT,
      senderName: request.senderBundleName,
      traceId: this.buildTraceId(),
      messageVersion: AI_DISPATCH_MESSAGE_VERSION,
      businessName: AI_DISPATCH_BUSINESS_NAME,
    };
    const result: AiDispatchRequestPayload = {
      session,
      content,
    };
    return result;
  }

  private buildTraceId(): string {
    const time = Date.now().toString(36);
    const random = Math.floor(Math.random() * 0xFFFFFF).toString(36);
    return `${time}${random}`;
  }

  private parseJsonObject(raw: string): Object | null {
    const text = raw.trim();
    if (text.length === 0) {
      return null;
    }
    try {
      const parsed = JSON.parse(text) as Object;
      if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) {
        return null;
      }
      return parsed;
    } catch (_error) {
      return null;
    }
  }

  private async sendProcessMessage(dispatchPayloadText: string, callback: rpc.IRemoteObject): Promise<void> {
    const remote = this.remote;
    if (!remote) {
      throw new Error('AiDispatch service not connected');
    }
    const data = rpc.MessageSequence.create();
    const reply = rpc.MessageSequence.create();
    const option = new rpc.MessageOption();
    try {
      data.writeInterfaceToken(AI_DISPATCH_BASE_SERVICE_DESCRIPTOR);
      writeSlicedString(data, dispatchPayloadText);
      data.writeRemoteObject(callback);
      const requestResult = await remote.sendMessageRequest(
        AI_DISPATCH_PROCESS_MESSAGE_CODE,
        data,
        reply,
        option
      );
      if (requestResult.errCode !== 0) {
        throw new Error(`errCode=${requestResult.errCode}`);
      }
      reply.readException();
    } finally {
      data.reclaim();
      reply.reclaim();
    }
  }
}
