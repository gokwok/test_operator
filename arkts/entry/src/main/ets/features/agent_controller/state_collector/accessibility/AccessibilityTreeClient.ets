import type { BusinessError } from '@kit.BasicServicesKit';
import commonEventManager from '@ohos.commonEventManager';
import accessibility from '@ohos.accessibility';
import config from '@ohos.accessibility.config';
import bundleManager from '@ohos.bundle.bundleManager';

import {
  ACCESS_TREE_ABILITY_NAME,
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
  private static selfBundleName: string | null = null;
  private static selfAbilityName: string | null = null;
  private static selfAbilityId: string | null = null;

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

  static async requestWindowsJson(
    params: AccessibilityTreeRequestOptions,
    timeoutMs: number = DEFAULT_TIMEOUT_MS
  ): Promise<string> {
    const request: AccessibilityTreeRequestOptions = {
      displayId: params.displayId,
      windowId: params.windowId,
      bundleName: params.bundleName,
      maxDepth: params.maxDepth,
      maxNodes: params.maxNodes,
      windowsOnly: true
    };
    const response = await AccessibilityTreeClient.requestTree(request, timeoutMs);
    if (!response.ok) {
      throw new Error(response.error ?? 'accessibility_windows_failed');
    }
    return response.data ?? '';
  }

  static async requestTreeListJson(
    params: AccessibilityTreeRequestOptions,
    timeoutMs: number = DEFAULT_TIMEOUT_MS
  ): Promise<string> {
    const request: AccessibilityTreeRequestOptions = {
      displayId: params.displayId,
      windowId: params.windowId,
      bundleName: params.bundleName,
      maxDepth: params.maxDepth,
      maxNodes: params.maxNodes,
      output: 'list'
    };
    const response = await AccessibilityTreeClient.requestTree(request, timeoutMs);
    if (!response.ok) {
      throw new Error(response.error ?? 'accessibility_tree_list_failed');
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
      maxNodes: params.maxNodes,
      windowsOnly: params.windowsOnly,
      includeAttributes: params.includeAttributes,
      output: params.output
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
      data
    };
    const bundleName = AccessibilityTreeClient.resolveSelfBundleName();
    if (bundleName.length > 0) {
      publishData.bundleName = bundleName;
    }
    commonEventManager.publish(ACCESS_TREE_REQUEST_EVENT, publishData, (_err: BusinessError | null) => {
      void _err;
    });
  }

  private static async enableExtensionBestEffort(): Promise<string> {
    try {
      const bundleName = AccessibilityTreeClient.resolveSelfBundleName();
      if (bundleName.length === 0) {
        return 'enable_failed_bundle_unknown';
      }
      const abilityId = AccessibilityTreeClient.resolveAccessibilityAbilityId(bundleName);
      if (abilityId.length > 0) {
        await config.enableAbility(abilityId, DEFAULT_CAPABILITIES);
        return '';
      }
      const abilityName = AccessibilityTreeClient.resolveAccessibilityAbilityName();
      if (abilityName.length === 0) {
        return 'enable_failed_ability_unknown';
      }
      const name = `${bundleName}/${abilityName}`;
      await config.enableAbility(name, DEFAULT_CAPABILITIES);
      return '';
    } catch (e) {
      const err = e as BusinessError;
      if (err && err.code === 9300002) {
        return '';
      }
      if (err && typeof err.code === 'number') {
        const bundleName = AccessibilityTreeClient.resolveSelfBundleName();
        const abilityId = AccessibilityTreeClient.resolveAccessibilityAbilityId(bundleName);
        if (abilityId.length > 0) {
          return `enable_failed_${err.code}@${abilityId}`;
        }
        const abilityName = AccessibilityTreeClient.resolveAccessibilityAbilityName();
        if (bundleName.length > 0 && abilityName.length > 0) {
          return `enable_failed_${err.code}@${bundleName}/${abilityName}`;
        }
        return `enable_failed_${err.code}`;
      }
      return 'enable_failed';
    }
  }

  private static resolveSelfBundleName(): string {
    if (AccessibilityTreeClient.selfBundleName !== null) {
      return AccessibilityTreeClient.selfBundleName;
    }
    let name = '';
    try {
      const info = bundleManager.getBundleInfoForSelfSync(bundleManager.BundleFlag.GET_BUNDLE_INFO_DEFAULT);
      name = info.name;
    } catch (_e) {
      name = '';
    }
    AccessibilityTreeClient.selfBundleName = name;
    return name;
  }

  private static resolveAccessibilityAbilityName(): string {
    if (AccessibilityTreeClient.selfAbilityName !== null) {
      return AccessibilityTreeClient.selfAbilityName;
    }
    let resolved = '';
    try {
      const flags =
        bundleManager.BundleFlag.GET_BUNDLE_INFO_WITH_HAP_MODULE |
        bundleManager.BundleFlag.GET_BUNDLE_INFO_WITH_EXTENSION_ABILITY;
      const info = bundleManager.getBundleInfoForSelfSync(flags);
      const modules = info.hapModulesInfo ?? [];
      let fallback = '';
      for (const moduleInfo of modules) {
        const extensions = moduleInfo.extensionAbilitiesInfo ?? [];
        for (const extensionInfo of extensions) {
          if (extensionInfo.extensionAbilityType !== bundleManager.ExtensionAbilityType.ACCESSIBILITY) {
            continue;
          }
          const name = extensionInfo.name;
          if (name.length === 0) {
            continue;
          }
          if (name === ACCESS_TREE_ABILITY_NAME || name.endsWith(`.${ACCESS_TREE_ABILITY_NAME}`)) {
            resolved = name;
            break;
          }
          if (fallback.length === 0) {
            fallback = name;
          }
        }
        if (resolved.length > 0) {
          break;
        }
      }
      if (resolved.length === 0) {
        resolved = fallback;
      }
    } catch (_e) {
      resolved = '';
    }
    if (resolved.length === 0) {
      resolved = ACCESS_TREE_ABILITY_NAME;
    }
    AccessibilityTreeClient.selfAbilityName = resolved;
    return resolved;
  }

  private static resolveAccessibilityAbilityId(bundleName: string): string {
    if (AccessibilityTreeClient.selfAbilityId !== null) {
      return AccessibilityTreeClient.selfAbilityId;
    }
    let resolved = '';
    try {
      const list = accessibility.getAccessibilityExtensionListSync('all', 'install');
      for (const info of list) {
        if (info.bundleName !== bundleName) {
          continue;
        }
        const id = info.id;
        if (id && id.indexOf('/') > 0) {
          resolved = id;
          break;
        }
        const name = info.name;
        if (name && name.indexOf('/') > 0) {
          resolved = name;
          break;
        }
        if (name) {
          resolved = `${bundleName}/${name}`;
          break;
        }
      }
    } catch (_e) {
      resolved = '';
    }
    AccessibilityTreeClient.selfAbilityId = resolved;
    return resolved;
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
