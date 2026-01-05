import bundleManager from '@ohos.bundle.bundleManager';
import bundleResourceManager from '@ohos.bundle.bundleResourceManager';
import preferences from '@ohos.data.preferences';
import image from '@ohos.multimedia.image';
import type { common } from '@kit.AbilityKit';
import type { AppTargetDescriptor } from '../types';

const APP_NAME_WHITELIST: string[] = [
  // Social & Messaging
  '微信',
  'QQ',
  '微博',
  // E-commerce
  '淘宝',
  '京东',
  '拼多多',
  '淘宝闪购',
  '京东秒送',
  // Lifestyle & Social
  '小红书',
  '知乎',
  // Maps & Navigation
  '高德地图',
  '百度地图',
  // Food & Services
  '美团',
  '美团外卖',
  '大众点评',
  '肯德基',
  // Travel
  '铁路12306',
  '12306',
  '滴滴出行',
  // Video & Entertainment
  'bilibili',
  '抖音',
  '快手',
  '腾讯视频',
  '爱奇艺',
  '芒果TV',
  // Music & Audio
  'QQ音乐',
  '汽水音乐',
  '喜马拉雅',
  // Productivity
  '飞书',
  // AI & Tools
  '豆包',
  // News & Information
  '今日头条',

  // HarmonyOS 第三方应用
  '百度',
  '阿里巴巴',
  'WPS',
  '企业微信',
  '同程',
  '同程旅行',
  '唯品会',
  '支付宝',
  'UC浏览器',
  '闲鱼',
  '转转',
  '迅雷',
  '搜狗输入法',
  '扫描全能王',
  '美图秀秀',
  '58同城',
  '得物',
  '海底捞',
  '中国移动',
  '中国联通',
  '国家税务总局',
  '建设银行',
  '快手极速版',

  // HarmonyOS 系统应用 - 工具类
  '浏览器',
  '计算器',
  '日历',
  '相机',
  '时钟',
  '云盘',
  '云空间',
  '邮件',
  '文件管理器',
  '文件',
  '查找设备',
  '查找手机',
  '录音机',
  '录音',
  '录屏',
  '截屏',
  '笔记',
  '备忘录',

  // HarmonyOS 系统应用 - 媒体类
  '相册',
  '图库',

  // HarmonyOS 系统应用 - 通讯类
  '联系人',
  '通讯录',
  '短信',
  '信息',
  '电话',
  '拨号',

  // HarmonyOS 系统应用 - 设置类
  '设置',
  '系统设置',
  'AndroidSystemSettings',
  'Android System Settings',
  'Android  System Settings',
  'Android-System-Settings',
  'Settings',

  // HarmonyOS 系统应用 - 生活服务
  '健康',
  '运动健康',
  '地图',
  '华为地图',
  '钱包',
  '华为钱包',
  '智慧生活',
  '智能助手',
  '小艺',

  // HarmonyOS 服务
  '应用市场',
  '华为应用市场',
  '音乐',
  '华为音乐',
  '主题',
  '主题管理',
  '天气',
  '华为天气',
  '视频',
  '华为视频',
  '阅读',
  '华为阅读',
  '游戏中心',
  '华为游戏中心',
  '搜索',
  '华为搜索',
  '指南针',
  '会员中心',
  '我的华为',
  '华为会员',
];

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

  static async listAppNames(ctx: common.Context): Promise<string[]> {
    await AppState.ensureAppCache(ctx);
    return AppState.filterAppNamesByWhitelist(AppState.cachedAppNames);
  }

  static async getLaunchTargetByAppName(appName: string, ctx: common.Context): Promise<AppLaunchTarget> {
    const bundleName = await AppState.resolveAppIdByName(appName, ctx);
    const want = await bundleManager.getLaunchWantForBundle(bundleName);
    const abilityName = want.abilityName?.trim() ?? '';
    if (abilityName.length === 0) {
      throw new Error(`Launch ability not found for bundle: ${bundleName}`);
    }

    return { bundleName, abilityName };
  }

  static async listAppTargets(ctx: common.Context): Promise<Array<AppTargetDescriptor>> {
    await AppState.ensureAppCache(ctx);
    const names = AppState.cachedAppNames.slice();
    const targets: Array<AppTargetDescriptor> = [];
    for (let i = 0; i < names.length; i++) {
      const displayName = names[i];
      let bundleName = '';
      try {
        bundleName = await AppState.resolveAppIdByName(displayName, ctx);
      } catch (_e) {
        continue;
      }
      let abilityName = '';
      try {
        const want = await bundleManager.getLaunchWantForBundle(bundleName);
        abilityName = want.abilityName?.trim() ?? '';
      } catch (_e) {
        // ignore
      }
      if (abilityName.length === 0) {
        abilityName = 'EntryAbility';
      }
      targets.push({ displayName, bundleName, abilityName });
    }
    return targets;
  }

  static async getIconByAppName(appName: string, ctx: common.Context): Promise<image.PixelMap> {
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

  private static async ensureAppCache(ctx: common.Context): Promise<void> {
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

  private static async refreshAppCache(ctx: common.Context): Promise<void> {
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

  private static async refreshAppCacheOnce(ctx: common.Context): Promise<void> {
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

  private static async resolveAppIdByName(appName: string, ctx: common.Context): Promise<string> {
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

  private static async loadDiskCache(ctx: common.Context): Promise<boolean> {
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

  private static async saveDiskCache(ctx: common.Context): Promise<void> {
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

  private static filterAppNamesByWhitelist(names: string[]): string[] {
    if (APP_NAME_WHITELIST.length === 0) {
      return names.slice();
    }

    const normalizedToNames = new Map<string, string[]>();
    for (let i = 0; i < names.length; i++) {
      const name = names[i];
      const normalized = AppState.normalizeDisplayNameForWhitelist(name);
      const existing = normalizedToNames.get(normalized);
      if (existing) {
        existing.push(name);
      } else {
        normalizedToNames.set(normalized, [name]);
      }
    }

    const out: string[] = [];
    const seen = new Set<string>();
    for (let i = 0; i < APP_NAME_WHITELIST.length; i++) {
      const allowed = APP_NAME_WHITELIST[i].trim();
      if (allowed.length === 0) {
        continue;
      }
      const matches = normalizedToNames.get(allowed);
      if (!matches) {
        continue;
      }
      for (let j = 0; j < matches.length; j++) {
        const value = matches[j];
        if (seen.has(value)) {
          continue;
        }
        seen.add(value);
        out.push(value);
      }
    }
    return out;
  }

  private static normalizeDisplayNameForWhitelist(name: string): string {
    const trimmed = name.trim();
    if (!trimmed.endsWith(')')) {
      return trimmed;
    }
    const openIndex = trimmed.lastIndexOf('(');
    if (openIndex < 0) {
      return trimmed;
    }
    return trimmed.slice(0, openIndex).trim();
  }

}
