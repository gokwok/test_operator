import type { BusinessError } from '@kit.BasicServicesKit';
import commonEventManager from '@ohos.commonEventManager';
import accessibility from '@ohos.accessibility';
import config from '@ohos.accessibility.config';

import {
  ACCESS_TREE_ABILITY_NAME,
  ACCESS_TREE_BUNDLE_NAME,
  ACCESS_TREE_REQUEST_EVENT,
  ACCESS_TREE_RESPONSE_EVENT,
  AccessibilityTreeRequestOptions,
  AccessibilityTreeRequestPayload,
  AccessibilityTreeResponsePayload
} from './AccessibilityTreeProtocol';

interface CommonEventDataSafe {
  data?: string;
}

interface CommonEventPublishDataSafe {
  bundleName?: string;
  data?: string;
}

interface PendingRequest {
  resolve: (value: AccessibilityTreeResponsePayload) => void;
  reject: (reason: Error) => void;
  timeoutId: number;
}

const DEFAULT_TIMEOUT_MS: number = 6000;
const DEFAULT_CAPABILITIES: Array<accessibility.Capability> = ['retrieve'];

export class AccessibilityTreeClient {
  private static subscriber: commonEventManager.CommonEventSubscriber | null = null;
  private static pending: Map<string, PendingRequest> = new Map();

  static async requestTreeJson(
    params: AccessibilityTreeRequestOptions,
    timeoutMs: number = DEFAULT_TIMEOUT_MS
  ): Promise<string> {
    const response = await AccessibilityTreeClient.requestTree(params, timeoutMs);
    if (!response.ok) {
      throw new Error(response.error ?? 'accessibility_tree_failed');
    }
    return response.data ?? '';
  }

  private static async requestTree(
    params: AccessibilityTreeRequestOptions,
    timeoutMs: number
  ): Promise<AccessibilityTreeResponsePayload> {
    await AccessibilityTreeClient.ensureSubscriber();
    const enableError = await AccessibilityTreeClient.enableExtensionBestEffort();
    const requestId = AccessibilityTreeClient.buildRequestId();
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
        AccessibilityTreeClient.pending.delete(requestId);
        const suffix = enableError.length > 0 ? `:${enableError}` : '';
        reject(new Error(`accessibility_tree_timeout${suffix}`));
      }, timeoutMs);
      AccessibilityTreeClient.pending.set(requestId, {
        resolve,
        reject,
        timeoutId
      });
    });
    AccessibilityTreeClient.publishRequest(payload);
    return await response;
  }

  private static async ensureSubscriber(): Promise<void> {
    if (AccessibilityTreeClient.subscriber) {
      return;
    }
    const info: commonEventManager.CommonEventSubscribeInfo = {
      events: [ACCESS_TREE_RESPONSE_EVENT]
    };
    const subscriber = await commonEventManager.createSubscriber(info);
    AccessibilityTreeClient.subscriber = subscriber;
    commonEventManager.subscribe(subscriber, (_err: BusinessError | null, data: object) => {
      void _err;
      AccessibilityTreeClient.handleResponse(data);
    });
  }

  private static handleResponse(data: object): void {
    const payloadText = AccessibilityTreeClient.readEventData(data);
    if (payloadText.length === 0) {
      return;
    }
    let parsed: AccessibilityTreeResponsePayload | null = null;
    try {
      parsed = JSON.parse(payloadText) as AccessibilityTreeResponsePayload;
    } catch (_e) {
      return;
    }
    const requestId = parsed?.requestId ?? '';
    if (requestId.length === 0) {
      return;
    }
    const pending = AccessibilityTreeClient.pending.get(requestId);
    if (!pending) {
      return;
    }
    clearTimeout(pending.timeoutId);
    AccessibilityTreeClient.pending.delete(requestId);
    const ok = parsed?.ok ?? false;
    const error = parsed?.error ?? '';
    const payload = parsed?.data ?? '';
    const truncated = parsed?.truncated ?? false;
    pending.resolve({
      requestId,
      ok,
      data: payload.length > 0 ? payload : undefined,
      error: error.length > 0 ? error : undefined,
      truncated
    });
  }

  private static publishRequest(payload: AccessibilityTreeRequestPayload): void {
    const data = JSON.stringify(payload);
    const publishData: CommonEventPublishDataSafe = {
      bundleName: ACCESS_TREE_BUNDLE_NAME,
      data
    };
    commonEventManager.publish(ACCESS_TREE_REQUEST_EVENT, publishData, (_err: BusinessError | null) => {
      void _err;
    });
  }

  private static async enableExtensionBestEffort(): Promise<string> {
    try {
      const name = `${ACCESS_TREE_BUNDLE_NAME}/${ACCESS_TREE_ABILITY_NAME}`;
      await config.enableAbility(name, DEFAULT_CAPABILITIES);
      return '';
    } catch (e) {
      const err = e as BusinessError;
      if (err && err.code === 9300002) {
        return '';
      }
      if (err && typeof err.code === 'number') {
        return `enable_failed_${err.code}`;
      }
      return 'enable_failed';
    }
  }

  private static buildRequestId(): string {
    const rand = Math.floor(Math.random() * 100000);
    return `acc_${Date.now()}_${rand}`;
  }

  private static readEventData(data: object): string {
    const safe = data as CommonEventDataSafe;
    const text = safe.data;
    if (typeof text === 'string') {
      return text;
    }
    return '';
  }
}
