import bundleManager from '@ohos.bundle.bundleManager';

export interface AppListItem {
  bundleName: string;
  versionName: string;
  versionCode: number;
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

  private static buildItem(info: bundleManager.BundleInfo): AppListItem {
    return {
      bundleName: info.name,
      versionName: info.versionName,
      versionCode: Number(info.versionCode),
    };
  }

  private static resolveAppLabel(info: bundleManager.BundleInfo): string {
    const candidate = info as bundleManager.BundleInfo & BundleInfoNameCandidate;

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
