import type { BusinessError } from '@kit.BasicServicesKit';
import commonEventManager, {
  CommonEventData,
  CommonEventPublishData,
  CommonEventSubscribeInfo,
  CommonEventSubscriber
} from '@ohos.commonEventManager';
import accessibility from '@ohos.accessibility';
import config from '@ohos.accessibility.config';

import {
  ACCESS_TREE_ABILITY_NAME,
  ACCESS_TREE_BUNDLE_NAME,
  ACCESS_TREE_REQUEST_EVENT,
  ACCESS_TREE_RESPONSE_EVENT,
  AccessibilityTreeRequestPayload,
  AccessibilityTreeResponsePayload
} from './AccessibilityTreeProtocol';

type EventParamValue = string | number | boolean | undefined;
type EventParams = Record<string, EventParamValue>;

interface PendingRequest {
  resolve: (value: AccessibilityTreeResponsePayload) => void;
  reject: (reason: Error) => void;
  timeoutId: number;
}

const DEFAULT_TIMEOUT_MS: number = 6000;
const DEFAULT_CAPABILITIES: Array<accessibility.Capability> = ['retrieve'];

export class AccessibilityTreeClient {
  private static subscriber: CommonEventSubscriber | null = null;
  private static pending: Map<string, PendingRequest> = new Map();

  static async requestTreeJson(
    params: Omit<AccessibilityTreeRequestPayload, 'requestId'>,
    timeoutMs: number = DEFAULT_TIMEOUT_MS
  ): Promise<string> {
    const response = await this.requestTree(params, timeoutMs);
    if (!response.ok) {
      throw new Error(response.error ?? 'accessibility_tree_failed');
    }
    return response.data ?? '';
  }

  private static async requestTree(
    params: Omit<AccessibilityTreeRequestPayload, 'requestId'>,
    timeoutMs: number
  ): Promise<AccessibilityTreeResponsePayload> {
    await this.ensureSubscriber();
    await this.enableExtensionBestEffort();
    const requestId = this.buildRequestId();
    const payload: AccessibilityTreeRequestPayload = {
      requestId,
      displayId: params.displayId,
      windowId: params.windowId,
      bundleName: params.bundleName,
      maxDepth: params.maxDepth,
      maxNodes: params.maxNodes
    };
    const response = new Promise<AccessibilityTreeResponsePayload>((resolve, reject) => {
      const timeoutId: number = setTimeout(() => {
        this.pending.delete(requestId);
        reject(new Error('accessibility_tree_timeout'));
      }, timeoutMs);
      this.pending.set(requestId, {
        resolve,
        reject,
        timeoutId
      });
    });
    this.publishRequest(payload);
    return await response;
  }

  private static async ensureSubscriber(): Promise<void> {
    if (this.subscriber) {
      return;
    }
    const info: CommonEventSubscribeInfo = {
      events: [ACCESS_TREE_RESPONSE_EVENT]
    };
    const subscriber = await commonEventManager.createSubscriber(info);
    this.subscriber = subscriber;
    commonEventManager.subscribe(subscriber, (data: CommonEventData) => {
      this.handleResponse(data);
    });
  }

  private static handleResponse(data: CommonEventData): void {
    const params = this.toEventParams(data.parameters);
    const requestId = this.readString(params, 'requestId');
    if (requestId.length === 0) {
      return;
    }
    const pending = this.pending.get(requestId);
    if (!pending) {
      return;
    }
    clearTimeout(pending.timeoutId);
    this.pending.delete(requestId);
    const ok = this.readBoolean(params, 'ok', false);
    const error = this.readString(params, 'error');
    const payload = this.readString(params, 'data');
    const truncated = this.readBoolean(params, 'truncated', false);
    pending.resolve({
      requestId,
      ok,
      data: payload.length > 0 ? payload : undefined,
      error: error.length > 0 ? error : undefined,
      truncated
    });
  }

  private static publishRequest(payload: AccessibilityTreeRequestPayload): void {
    const params: EventParams = {
      requestId: payload.requestId,
      displayId: payload.displayId,
      windowId: payload.windowId,
      bundleName: payload.bundleName,
      maxDepth: payload.maxDepth,
      maxNodes: payload.maxNodes
    };
    const publishData: CommonEventPublishData = {
      bundleName: ACCESS_TREE_BUNDLE_NAME,
      parameters: this.pruneParams(params)
    };
    commonEventManager.publish(ACCESS_TREE_REQUEST_EVENT, publishData, (_err: BusinessError | null) => {
      void _err;
    });
  }

  private static async enableExtensionBestEffort(): Promise<void> {
    try {
      const name = `${ACCESS_TREE_BUNDLE_NAME}/${ACCESS_TREE_ABILITY_NAME}`;
      await config.enableAbility(name, DEFAULT_CAPABILITIES);
    } catch (e) {
      const err = e as BusinessError;
      if (err && err.code === 9300002) {
        return;
      }
    }
  }

  private static buildRequestId(): string {
    const rand = Math.floor(Math.random() * 100000);
    return `acc_${Date.now()}_${rand}`;
  }

  private static toEventParams(raw?: EventParams): EventParams {
    if (!raw) {
      return {};
    }
    return raw;
  }

  private static pruneParams(params: EventParams): EventParams {
    const filtered: EventParams = {};
    for (const key of Object.keys(params)) {
      const value = params[key];
      if (value !== undefined && value !== '') {
        filtered[key] = value;
      }
    }
    return filtered;
  }

  private static readString(params: EventParams, key: string): string {
    const value = params[key];
    if (typeof value === 'string') {
      return value;
    }
    if (typeof value === 'number') {
      return String(value);
    }
    return '';
  }

  private static readBoolean(params: EventParams, key: string, fallback: boolean): boolean {
    const value = params[key];
    if (typeof value === 'boolean') {
      return value;
    }
    if (typeof value === 'number') {
      return value !== 0;
    }
    if (typeof value === 'string') {
      return value === '1' || value.toLowerCase() === 'true';
    }
    return fallback;
  }
}
