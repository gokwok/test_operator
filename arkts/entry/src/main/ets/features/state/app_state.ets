import bundleManager from '@ohos.bundle.bundleManager';
import bundleResourceManager from '@ohos.bundle.bundleResourceManager';
import image from '@ohos.multimedia.image';
import type { common } from '@kit.AbilityKit';

export interface AppLaunchTarget {
  bundleName: string;
  abilityName: string;
}

export class appInfo {
  appId: string = '';
  appLabel: string | ResourceStr | undefined = '';
  appIcon: image.PixelMap | undefined = undefined;
}

export class AppState {
  private static readonly appNameToAppId: Map<string, string> = new Map<string, string>();
  private static readonly appIdToInfo: Map<string, appInfo> = new Map<string, appInfo>();
  private static readonly cacheTtlMs: number = 2000;
  private static cacheUpdatedAtMs: number = 0;
  private static cachedAppNames: string[] = [];

  static async listAppNames(_ctx: common.UIAbilityContext): Promise<string[]> {
    await AppState.ensureAppCache();
    return AppState.cachedAppNames.slice();
  }

  static async getLaunchTargetByAppName(appName: string, _ctx: common.UIAbilityContext): Promise<AppLaunchTarget> {
    const bundleName = await AppState.resolveAppIdByName(appName);
    const want = await bundleManager.getLaunchWantForBundle(bundleName);
    const abilityName = want.abilityName?.trim() ?? '';
    if (abilityName.length === 0) {
      throw new Error(`Launch ability not found for bundle: ${bundleName}`);
    }

    return { bundleName, abilityName };
  }

  static async getIconByAppName(appName: string, _ctx: common.UIAbilityContext): Promise<image.PixelMap> {
    const appId = await AppState.resolveAppIdByName(appName);
    await AppState.ensureAppCache();

    const info = AppState.appIdToInfo.get(appId);
    const pixelMap = info?.appIcon;
    if (!pixelMap) {
      throw new Error(`App icon not found: ${appName}`);
    }
    return pixelMap;
  }

  private static async ensureAppCache(): Promise<void> {
    const now = Date.now();
    if (AppState.cachedAppNames.length > 0 && now - AppState.cacheUpdatedAtMs < AppState.cacheTtlMs) {
      return;
    }
    await AppState.refreshAppCache();
  }

  private static async refreshAppCache(): Promise<void> {
    const flags: number = bundleManager.BundleFlag.GET_BUNDLE_INFO_WITH_APPLICATION;
    const bundles = await bundleManager.getAllBundleInfo(flags);

    const candidates: Array<{ info: appInfo; displayName: string }> = [];
    for (let i = 0; i < bundles.length; i++) {
      const bundleInfo = bundles[i];
      const bundleName = bundleInfo.name?.trim() ?? '';
      if (bundleName.length === 0) {
        continue;
      }

      const info = new appInfo();
      info.appId = bundleName;

      const appFlags: number = bundleManager.ApplicationFlag.GET_APPLICATION_INFO_DEFAULT |
        bundleManager.ApplicationFlag.GET_APPLICATION_INFO_WITH_DISABLE;

      let appIndex: number = 0;
      let applicationName: string = bundleName;
      try {
        const applicationInfo = await bundleManager.getApplicationInfo(bundleName, appFlags);
        appIndex = applicationInfo.appIndex;
        applicationName = applicationInfo.name;
      } catch (_e) {
        candidates.push({ info, displayName: bundleName });
        continue;
      }

      try {
        const iconFlags: number = bundleResourceManager.ResourceFlag.GET_RESOURCE_INFO_WITH_DRAWABLE_DESCRIPTOR;
        const resourceInfo = bundleResourceManager.getBundleResourceInfo(applicationName, iconFlags, appIndex);
        const pixelMap = resourceInfo?.drawableDescriptor?.getPixelMap();
        if (pixelMap) {
          info.appIcon = pixelMap;
        }
      } catch (_e) {
        // ignore
      }

      try {
        const labelFlags: number = bundleResourceManager.ResourceFlag.GET_RESOURCE_INFO_ALL;
        const resourceInfo = bundleResourceManager.getBundleResourceInfo(bundleName, labelFlags, appIndex);
        info.appLabel = resourceInfo?.label;
      } catch (_e) {
        // ignore
      }

      const label = AppState.normalizeAppLabel(info.appLabel);
      const displayName = label.length > 0 ? label : bundleName;
      candidates.push({ info, displayName });
    }

    const duplicateCounts = new Map<string, number>();
    for (let i = 0; i < candidates.length; i++) {
      const name = candidates[i].displayName;
      duplicateCounts.set(name, (duplicateCounts.get(name) ?? 0) + 1);
    }

    const names: string[] = [];
    AppState.appNameToAppId.clear();
    AppState.appIdToInfo.clear();
    for (let i = 0; i < candidates.length; i++) {
      const candidate = candidates[i];
      const count = duplicateCounts.get(candidate.displayName) ?? 0;
      const formattedName = AppState.formatAppName(candidate.info, candidate.displayName, count);
      names.push(formattedName);
      AppState.appNameToAppId.set(formattedName, candidate.info.appId);
      AppState.appIdToInfo.set(candidate.info.appId, candidate.info);
    }

    names.sort((a, b) => AppState.compareAppNames(a, b));
    AppState.cachedAppNames = AppState.uniqueStrings(names);
    AppState.cacheUpdatedAtMs = Date.now();
  }

  private static normalizeAppLabel(label: string | ResourceStr | undefined): string {
    if (!label) {
      return '';
    }
    if (typeof label === 'string') {
      const normalized = label.trim();
      return AppState.isResourceLikeLabel(normalized) ? '' : normalized;
    }
    const normalized = String(label).trim();
    return AppState.isResourceLikeLabel(normalized) ? '' : normalized;
  }

  private static async resolveAppIdByName(appName: string): Promise<string> {
    const normalized = appName.trim();
    if (normalized.length === 0) {
      throw new Error('appName is empty');
    }

    await AppState.ensureAppCache();

    if (AppState.appIdToInfo.has(normalized)) {
      return normalized;
    }

    const cached = AppState.appNameToAppId.get(normalized);
    if (cached) {
      return cached;
    }

    const extracted = AppState.extractBundleNameSuffix(normalized);
    if (extracted.length > 0 && AppState.appIdToInfo.has(extracted)) {
      return extracted;
    }

    throw new Error(`App not found: ${appName}`);
  }

  private static formatAppName(info: appInfo, displayName: string, duplicateCount: number): string {
    if (duplicateCount <= 1) {
      return displayName;
    }
    const label = AppState.normalizeAppLabel(info.appLabel);
    if (label.length > 0) {
      return `${label} (${info.appId})`;
    }
    return info.appId;
  }

  private static isResourceLikeLabel(label: string): boolean {
    return label.startsWith('$');
  }

  private static compareAppNames(a: string, b: string): number {
    const aIsBundle = AppState.looksLikeBundleName(a);
    const bIsBundle = AppState.looksLikeBundleName(b);
    if (aIsBundle !== bIsBundle) {
      return aIsBundle ? 1 : -1;
    }
    return a.localeCompare(b);
  }

  private static looksLikeBundleName(name: string): boolean {
    const normalized = name.trim();
    return normalized.startsWith('com.') ||
      normalized.startsWith('ohos.') ||
      normalized.startsWith('cn.') ||
      normalized.startsWith('net.') ||
      normalized.startsWith('org.');
  }

  private static extractBundleNameSuffix(input: string): string {
    if (!input.endsWith(')')) {
      return '';
    }
    const openIndex = input.lastIndexOf('(');
    if (openIndex < 0) {
      return '';
    }
    const extracted = input.slice(openIndex + 1, input.length - 1).trim();
    return extracted;
  }

  private static uniqueStrings(items: string[]): string[] {
    const out: string[] = [];
    for (let i = 0; i < items.length; i++) {
      const value = items[i];
      if (i > 0 && value === items[i - 1]) {
        continue;
      }
      out.push(value);
    }
    return out;
  }

}
