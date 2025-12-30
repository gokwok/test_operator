import bundleManager from '@ohos.bundle.bundleManager';
import image from '@ohos.multimedia.image';
import type { common } from '@kit.AbilityKit';

export interface AppListItem {
  bundleName: string;
  versionName: string;
  versionCode: number;
}

export interface AppLaunchTarget {
  bundleName: string;
  abilityName: string;
}

export interface AppResourceInfo {
  bundleName: string;
  moduleName: string;
  id: number;
}

export interface AppInfo {
  appId: string;
  appLabel: string;
  appIcon: AppResourceInfo | null;
}

interface ResourceIdCandidate {
  bundleName: string;
  moduleName: string;
  id: number | string;
}

interface ApplicationInfoCandidate {
  label?: string;
  labelResource?: ResourceIdCandidate;
  iconResource?: ResourceIdCandidate;
}

interface BundleInfoCandidate {
  appInfo?: ApplicationInfoCandidate;
}

interface AppInfoCandidate {
  appId: string;
  label: string;
  displayName: string;
  iconResource: AppResourceInfo | null;
}

export class AppState {
  private static readonly appNameToAppId: Map<string, string> = new Map<string, string>();
  private static readonly appIdToInfo: Map<string, AppInfo> = new Map<string, AppInfo>();
  private static readonly cacheTtlMs: number = 2000;
  private static cacheUpdatedAtMs: number = 0;
  private static cachedAppNames: string[] = [];

  static async listInstalledApps(): Promise<AppListItem[]> {
    const flags: number = bundleManager.BundleFlag.GET_BUNDLE_INFO_DEFAULT;
    const bundles: Array<bundleManager.BundleInfo> = await bundleManager.getAllBundleInfo(flags);
    const items: AppListItem[] = [];
    for (let i = 0; i < bundles.length; i++) {
      const info = bundles[i];
      const item = AppState.buildItem(info);
      const bundleName: string = item.bundleName;
      if (bundleName.length === 0) {
        continue;
      }
      items.push(item);
    }
    items.sort((a, b) => a.bundleName.localeCompare(b.bundleName));
    return items;
  }

  static async listAppNames(ctx: common.UIAbilityContext): Promise<string[]> {
    await AppState.ensureAppCache(ctx);
    return AppState.cachedAppNames.slice();
  }

  static async getLaunchTargetByAppName(appName: string, ctx: common.UIAbilityContext): Promise<AppLaunchTarget> {
    const bundleName = await AppState.resolveAppIdByName(appName, ctx);
    const want = await bundleManager.getLaunchWantForBundle(bundleName);
    const abilityName = want.abilityName?.trim() ?? '';
    if (abilityName.length === 0) {
      throw new Error(`Launch ability not found for bundle: ${bundleName}`);
    }

    return { bundleName, abilityName };
  }

  static async getIconByAppName(appName: string, ctx: common.UIAbilityContext): Promise<image.PixelMap> {
    const appId = await AppState.resolveAppIdByName(appName, ctx);
    await AppState.ensureAppCache(ctx);

    const info = AppState.appIdToInfo.get(appId);
    const iconResource = info?.appIcon;
    if (!iconResource) {
      throw new Error(`App icon not found: ${appName}`);
    }

    const content = await ctx.resourceManager.getMediaContent(iconResource);
    const buffer = content.buffer.slice(content.byteOffset, content.byteOffset + content.byteLength);
    const source = image.createImageSource(buffer);
    return await source.createPixelMap();
  }

  private static buildItem(info: bundleManager.BundleInfo): AppListItem {
    return {
      bundleName: info.name,
      versionName: info.versionName,
      versionCode: Number(info.versionCode),
    };
  }

  private static async ensureAppCache(ctx: common.UIAbilityContext): Promise<void> {
    const now = Date.now();
    if (AppState.cachedAppNames.length > 0 && now - AppState.cacheUpdatedAtMs < AppState.cacheTtlMs) {
      return;
    }
    await AppState.refreshAppCache(ctx);
  }

  private static async refreshAppCache(ctx: common.UIAbilityContext): Promise<void> {
    const flags: number = bundleManager.BundleFlag.GET_BUNDLE_INFO_DEFAULT |
      bundleManager.BundleFlag.GET_BUNDLE_INFO_WITH_APPLICATION;
    const bundles: Array<bundleManager.BundleInfo> = await bundleManager.getAllBundleInfo(flags);

    const candidates: AppInfoCandidate[] = [];
    for (let i = 0; i < bundles.length; i++) {
      const info = bundles[i];
      const appId = info.name?.trim() ?? '';
      if (appId.length === 0) {
        continue;
      }
      const appInfoCandidate = (info as BundleInfoCandidate).appInfo;
      const label = await AppState.resolveAppLabel(appInfoCandidate, ctx);
      const displayName = label.length > 0 ? label : appId;
      const iconResource = AppState.resolveAppIconResource(appInfoCandidate);
      candidates.push({ appId, label, displayName, iconResource });
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
      const formattedName = AppState.formatAppName(candidate, count);
      names.push(formattedName);
      AppState.appNameToAppId.set(formattedName, candidate.appId);
      AppState.appIdToInfo.set(candidate.appId, {
        appId: candidate.appId,
        appLabel: candidate.label,
        appIcon: candidate.iconResource,
      });
    }

    names.sort((a, b) => a.localeCompare(b));
    AppState.cachedAppNames = AppState.uniqueStrings(names);
    AppState.cacheUpdatedAtMs = Date.now();
  }

  private static async resolveAppLabel(
    appInfo: ApplicationInfoCandidate | undefined,
    ctx: common.UIAbilityContext
  ): Promise<string> {
    if (!appInfo) {
      return '';
    }
    const labelResource = appInfo.labelResource;
    if (labelResource) {
      const resolved = await AppState.resolveResourceString(labelResource, ctx);
      if (resolved.length > 0) {
        return resolved;
      }
    }

    const label = appInfo.label?.trim() ?? '';
    if (label.length > 0 && !AppState.isResourceLikeLabel(label)) {
      return label;
    }

    return '';
  }

  private static async resolveResourceString(resource: ResourceIdCandidate, ctx: common.UIAbilityContext): Promise<string> {
    try {
      const id = typeof resource.id === 'number' ? resource.id : Number(resource.id);
      if (!Number.isFinite(id) || id <= 0) {
        return '';
      }
      const normalized: AppResourceInfo = {
        bundleName: resource.bundleName,
        moduleName: resource.moduleName,
        id,
      };
      const value = (await ctx.resourceManager.getStringValue(normalized)).trim();
      if (value.length === 0) {
        return '';
      }
      if (AppState.isResourceLikeLabel(value)) {
        return '';
      }
      return value;
    } catch (_e) {
      return '';
    }
  }

  private static resolveAppIconResource(appInfo: ApplicationInfoCandidate | undefined): AppResourceInfo | null {
    if (!appInfo) {
      return null;
    }
    const resource = appInfo.iconResource;
    if (!resource) {
      return null;
    }

    const id = typeof resource.id === 'number' ? resource.id : Number(resource.id);
    if (!Number.isFinite(id) || id <= 0) {
      return null;
    }
    return {
      bundleName: resource.bundleName,
      moduleName: resource.moduleName,
      id,
    };
  }

  private static async resolveAppIdByName(appName: string, ctx: common.UIAbilityContext): Promise<string> {
    const normalized = appName.trim();
    if (normalized.length === 0) {
      throw new Error('appName is empty');
    }

    await AppState.ensureAppCache(ctx);

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

  private static formatAppName(candidate: AppInfoCandidate, duplicateCount: number): string {
    if (duplicateCount <= 1) {
      return candidate.displayName;
    }
    if (candidate.label.length > 0) {
      return `${candidate.label} (${candidate.appId})`;
    }
    return candidate.appId;
  }

  private static isResourceLikeLabel(label: string): boolean {
    return label.startsWith('$');
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
