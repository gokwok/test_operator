import bundleManager from '@ohos.bundle.bundleManager';
import type { Want } from '@kit.AbilityKit';

export interface AppListItem {
  bundleName: string;
  versionName: string;
  versionCode: number;
}

export interface AppLaunchTarget {
  bundleName: string;
  abilityName: string;
}

interface AppInfoNameCandidate {
  label?: string;
  name?: string;
}

interface BundleInfoNameCandidate {
  label?: string;
  appInfo?: AppInfoNameCandidate;
}

interface AppNameCandidate {
  bundleName: string;
  label: string;
  displayName: string;
}

interface BundleManagerLaunchWantApi {
  getLaunchWantForBundle?: (bundleName: string) => Promise<Want>;
}

interface WantNameCandidate {
  abilityName?: string;
}

export class AppState {
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

  static async listAppNames(): Promise<string[]> {
    const flags: number = bundleManager.BundleFlag.GET_BUNDLE_INFO_DEFAULT;
    const bundles: Array<bundleManager.BundleInfo> = await bundleManager.getAllBundleInfo(flags);

    const candidates: AppNameCandidate[] = [];
    for (let i = 0; i < bundles.length; i++) {
      const info = bundles[i];
      const item = AppState.buildItem(info);
      const bundleName: string = item.bundleName;
      if (bundleName.length === 0) {
        continue;
      }
      const label = AppState.resolveAppLabel(info);
      const displayName = label.length > 0 ? label : bundleName;
      candidates.push({ bundleName, label, displayName });
    }

    const duplicateCounts = new Map<string, number>();
    for (let i = 0; i < candidates.length; i++) {
      const name = candidates[i].displayName;
      duplicateCounts.set(name, (duplicateCounts.get(name) ?? 0) + 1);
    }

    const names: string[] = [];
    for (let i = 0; i < candidates.length; i++) {
      const candidate = candidates[i];
      const count = duplicateCounts.get(candidate.displayName) ?? 0;
      if (count <= 1) {
        names.push(candidate.displayName);
        continue;
      }
      if (candidate.label.length > 0) {
        names.push(`${candidate.label} (${candidate.bundleName})`);
        continue;
      }
      names.push(candidate.bundleName);
    }

    names.sort((a, b) => a.localeCompare(b));
    return AppState.uniqueStrings(names);
  }

  static async getLaunchTargetByAppName(appName: string): Promise<AppLaunchTarget> {
    const normalizedAppName = appName.trim();
    if (normalizedAppName.length === 0) {
      throw new Error('appName is empty');
    }

    const bundleName = await AppState.resolveBundleNameByAppName(normalizedAppName);

    const bm = bundleManager as BundleManagerLaunchWantApi;
    const getLaunchWantForBundle = bm.getLaunchWantForBundle;
    if (!getLaunchWantForBundle) {
      throw new Error('bundleManager.getLaunchWantForBundle is not available');
    }

    const want = await getLaunchWantForBundle(bundleName);
    const candidate = want as WantNameCandidate;
    const abilityName = candidate.abilityName?.trim() ?? '';
    if (abilityName.length === 0) {
      throw new Error(`Launch ability not found for bundle: ${bundleName}`);
    }

    return { bundleName, abilityName };
  }

  private static buildItem(info: bundleManager.BundleInfo): AppListItem {
    return {
      bundleName: info.name,
      versionName: info.versionName,
      versionCode: Number(info.versionCode),
    };
  }

  private static resolveAppLabel(info: bundleManager.BundleInfo): string {
    const candidate = info as BundleInfoNameCandidate;

    const directLabel = candidate.label?.trim() ?? '';
    if (directLabel.length > 0) {
      return directLabel;
    }

    const appInfo = candidate.appInfo;
    if (!appInfo) {
      return '';
    }

    const appLabel = appInfo.label?.trim() ?? '';
    if (appLabel.length > 0) {
      return appLabel;
    }

    const appName = appInfo.name?.trim() ?? '';
    return appName;
  }

  private static async resolveBundleNameByAppName(appName: string): Promise<string> {
    const flags: number = bundleManager.BundleFlag.GET_BUNDLE_INFO_DEFAULT;
    const bundles: Array<bundleManager.BundleInfo> = await bundleManager.getAllBundleInfo(flags);

    const direct = AppState.getBundleNameIfInstalled(bundles, appName);
    if (direct.length > 0) {
      return direct;
    }

    const extracted = AppState.extractBundleNameSuffix(appName);
    if (extracted.length > 0) {
      const bySuffix = AppState.getBundleNameIfInstalled(bundles, extracted);
      if (bySuffix.length > 0) {
        return bySuffix;
      }
    }

    const candidates: AppNameCandidate[] = [];
    for (let i = 0; i < bundles.length; i++) {
      const info = bundles[i];
      const item = AppState.buildItem(info);
      const bundleName: string = item.bundleName;
      if (bundleName.length === 0) {
        continue;
      }
      const label = AppState.resolveAppLabel(info);
      const displayName = label.length > 0 ? label : bundleName;
      candidates.push({ bundleName, label, displayName });
    }

    const duplicateCounts = new Map<string, number>();
    for (let i = 0; i < candidates.length; i++) {
      const name = candidates[i].displayName;
      duplicateCounts.set(name, (duplicateCounts.get(name) ?? 0) + 1);
    }

    let ambiguousLabelCount = 0;
    for (let i = 0; i < candidates.length; i++) {
      const candidate = candidates[i];
      const count = duplicateCounts.get(candidate.displayName) ?? 0;
      const finalName = AppState.formatAppName(candidate, count);
      if (finalName === appName) {
        return candidate.bundleName;
      }
      if (candidate.label.length > 0 && candidate.label === appName && count > 1) {
        ambiguousLabelCount += 1;
      }
    }

    if (ambiguousLabelCount > 0) {
      throw new Error(`App name is ambiguous: ${appName}. Use listAppNames() output.`);
    }
    throw new Error(`App not found: ${appName}`);
  }

  private static formatAppName(candidate: AppNameCandidate, duplicateCount: number): string {
    if (duplicateCount <= 1) {
      return candidate.displayName;
    }
    if (candidate.label.length > 0) {
      return `${candidate.label} (${candidate.bundleName})`;
    }
    return candidate.bundleName;
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

  private static getBundleNameIfInstalled(bundles: Array<bundleManager.BundleInfo>, bundleName: string): string {
    for (let i = 0; i < bundles.length; i++) {
      const info = bundles[i];
      if (info.name === bundleName) {
        return bundleName;
      }
    }
    return '';
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
