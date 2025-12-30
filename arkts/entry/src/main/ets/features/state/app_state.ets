import bundleManager from '@ohos.bundle.bundleManager';
import bundleResourceManager from '@ohos.bundle.bundleResourceManager';
import preferences from '@ohos.data.preferences';
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

class AppCacheCandidate {
  info: appInfo;
  displayName: string;

  constructor(info: appInfo, displayName: string) {
    this.info = info;
    this.displayName = displayName;
  }
}

class PersistedAppEntry {
  appId: string;
  label: string;
  displayName: string;

  constructor(appId: string, label: string, displayName: string) {
    this.appId = appId;
    this.label = label;
    this.displayName = displayName;
  }
}

class PersistedAppCache {
  schemaVersion: number;
  updatedAtMs: number;
  entries: PersistedAppEntry[];

  constructor(schemaVersion: number, updatedAtMs: number, entries: PersistedAppEntry[]) {
    this.schemaVersion = schemaVersion;
    this.updatedAtMs = updatedAtMs;
    this.entries = entries;
  }
}

export class AppState {
  private static readonly appNameToAppId: Map<string, string> = new Map<string, string>();
  private static readonly appIdToInfo: Map<string, appInfo> = new Map<string, appInfo>();
  private static readonly cacheTtlMs: number = 24 * 60 * 60 * 1000;
  private static cacheUpdatedAtMs: number = 0;
  private static cachedAppNames: string[] = [];
  private static diskCacheLoaded: boolean = false;
  private static refreshInFlight: Promise<void> | null = null;

  private static readonly diskSchemaVersion: number = 1;
  private static readonly prefsName: string = 'app_state_cache';
  private static readonly prefsKey: string = 'cache_v1';

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
    if (!info) {
      throw new Error(`App not found: ${appName}`);
    }

    if (info.appIcon) {
      return info.appIcon;
    }

    const appFlags: number = bundleManager.ApplicationFlag.GET_APPLICATION_INFO_DEFAULT |
      bundleManager.ApplicationFlag.GET_APPLICATION_INFO_WITH_DISABLE;

    const applicationInfo = await bundleManager.getApplicationInfo(appId, appFlags);
    const iconFlags: number = bundleResourceManager.ResourceFlag.GET_RESOURCE_INFO_WITH_DRAWABLE_DESCRIPTOR;
    const resourceInfo = bundleResourceManager.getBundleResourceInfo(applicationInfo.name, iconFlags, applicationInfo.appIndex);
    const pixelMap = resourceInfo?.drawableDescriptor?.getPixelMap();
    if (!pixelMap) {
      throw new Error(`App icon not found: ${appName}`);
    }
    info.appIcon = pixelMap;
    return pixelMap;
  }

  private static async ensureAppCache(ctx: common.UIAbilityContext): Promise<void> {
    const now = Date.now();
    if (AppState.cachedAppNames.length > 0 && now - AppState.cacheUpdatedAtMs < AppState.cacheTtlMs) {
      return;
    }

    if (!AppState.diskCacheLoaded) {
      AppState.diskCacheLoaded = true;
      const loaded = await AppState.loadDiskCache(ctx);
      if (loaded && AppState.cachedAppNames.length > 0 && now - AppState.cacheUpdatedAtMs < AppState.cacheTtlMs) {
        return;
      }
    }

    await AppState.refreshAppCache(ctx);
  }

  private static async refreshAppCache(ctx: common.UIAbilityContext): Promise<void> {
    if (AppState.refreshInFlight) {
      await AppState.refreshInFlight;
      return;
    }
    const task = AppState.refreshAppCacheOnce(ctx);
    AppState.refreshInFlight = task;
    try {
      await task;
    } finally {
      AppState.refreshInFlight = null;
    }
  }

  private static async refreshAppCacheOnce(ctx: common.UIAbilityContext): Promise<void> {
    const flags: number = bundleManager.BundleFlag.GET_BUNDLE_INFO_WITH_APPLICATION;
    const bundles = await bundleManager.getAllBundleInfo(flags);

    const candidates: AppCacheCandidate[] = [];
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
      try {
        const applicationInfo = await bundleManager.getApplicationInfo(bundleName, appFlags);
        appIndex = applicationInfo.appIndex;
      } catch (_e) {
        candidates.push(new AppCacheCandidate(info, bundleName));
        continue;
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
      candidates.push(new AppCacheCandidate(info, displayName));
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

    await AppState.saveDiskCache(ctx);
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

    await AppState.refreshAppCache(ctx);
    const retry = AppState.appNameToAppId.get(normalized) ?? AppState.appNameToAppId.get(appName);
    if (retry) {
      return retry;
    }
    if (AppState.appIdToInfo.has(normalized)) {
      return normalized;
    }
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

  private static async loadDiskCache(ctx: common.UIAbilityContext): Promise<boolean> {
    try {
      const prefs = await preferences.getPreferences(ctx, AppState.prefsName);
      const raw = String(await prefs.get(AppState.prefsKey, '')).trim();
      if (raw.length === 0) {
        return false;
      }

      const parsed = JSON.parse(raw) as PersistedAppCache;
      const loaded = AppState.parsePersistedCache(parsed);
      if (!loaded) {
        return false;
      }
      AppState.cacheUpdatedAtMs = loaded.updatedAtMs;
      AppState.applyPersistedEntries(loaded.entries);
      return AppState.cachedAppNames.length > 0;
    } catch (_e) {
      return false;
    }
  }

  private static parsePersistedCache(parsed: PersistedAppCache): PersistedAppCache | null {
    if (!parsed || typeof parsed.schemaVersion !== 'number') {
      return null;
    }
    if (parsed.schemaVersion !== AppState.diskSchemaVersion) {
      return null;
    }
    if (typeof parsed.updatedAtMs !== 'number' || !Number.isFinite(parsed.updatedAtMs) || parsed.updatedAtMs <= 0) {
      return null;
    }
    if (!Array.isArray(parsed.entries)) {
      return null;
    }
    const entries: PersistedAppEntry[] = [];
    for (let i = 0; i < parsed.entries.length; i++) {
      const entry = parsed.entries[i] as PersistedAppEntry;
      if (!entry || typeof entry.appId !== 'string' || typeof entry.displayName !== 'string' || typeof entry.label !== 'string') {
        continue;
      }
      const appId = entry.appId.trim();
      const displayName = entry.displayName.trim();
      if (appId.length === 0 || displayName.length === 0) {
        continue;
      }
      const label = entry.label.trim();
      entries.push(new PersistedAppEntry(appId, label, displayName));
    }
    return new PersistedAppCache(parsed.schemaVersion, parsed.updatedAtMs, entries);
  }

  private static applyPersistedEntries(entries: PersistedAppEntry[]): void {
    const names: string[] = [];
    AppState.appNameToAppId.clear();
    AppState.appIdToInfo.clear();

    for (let i = 0; i < entries.length; i++) {
      const entry = entries[i];
      const info = new appInfo();
      info.appId = entry.appId;
      info.appLabel = entry.label;
      names.push(entry.displayName);
      AppState.appNameToAppId.set(entry.displayName, entry.appId);
      AppState.appIdToInfo.set(entry.appId, info);
    }

    names.sort((a, b) => AppState.compareAppNames(a, b));
    AppState.cachedAppNames = AppState.uniqueStrings(names);
  }

  private static async saveDiskCache(ctx: common.UIAbilityContext): Promise<void> {
    try {
      const entries: PersistedAppEntry[] = [];
      for (let i = 0; i < AppState.cachedAppNames.length; i++) {
        const displayName = AppState.cachedAppNames[i];
        const appId = AppState.appNameToAppId.get(displayName);
        if (!appId) {
          continue;
        }
        const info = AppState.appIdToInfo.get(appId);
        const label = AppState.normalizeAppLabel(info?.appLabel);
        entries.push(new PersistedAppEntry(appId, label, displayName));
      }

      const cache = new PersistedAppCache(AppState.diskSchemaVersion, AppState.cacheUpdatedAtMs, entries);
      const raw = JSON.stringify(cache);
      const prefs = await preferences.getPreferences(ctx, AppState.prefsName);
      await prefs.put(AppState.prefsKey, raw);
      await prefs.flush();
    } catch (_e) {
      // ignore
    }
  }

}
