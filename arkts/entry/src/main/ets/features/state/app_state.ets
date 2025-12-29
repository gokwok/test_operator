import bundleManager from '@ohos.bundle.bundleManager';

export interface AppListItem {
  bundleName: string;
  versionName: string;
  versionCode: number;
}

export type AppListScope = 'all' | 'self';

export interface AppListResult {
  apps: AppListItem[];
  scope: AppListScope;
}

export class AppState {
  static async listInstalledApps(): Promise<AppListResult> {
    const flags: number = bundleManager.BundleFlag.GET_BUNDLE_INFO_DEFAULT;
    let bundles: Array<bundleManager.BundleInfo> = [];
    try {
      bundles = await bundleManager.getAllBundleInfo(flags);
    } catch (e) {
      if (AppState.isPermissionError(e)) {
        const selfApps = await AppState.listSelfApps(flags);
        return { apps: selfApps, scope: 'self' };
      }
      throw e;
    }
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
    return { apps: items, scope: 'all' };
  }

  private static async listSelfApps(flags: number): Promise<AppListItem[]> {
    const info = await bundleManager.getBundleInfoForSelf(flags);
    const item = AppState.buildItem(info);
    return item.bundleName.length === 0 ? [] : [item];
  }

  private static buildItem(info: bundleManager.BundleInfo): AppListItem {
    return {
      bundleName: info.name,
      versionName: info.versionName,
      versionCode: Number(info.versionCode),
    };
  }

  private static isPermissionError(error: unknown): boolean {
    if (typeof error !== 'object' || error === null) {
      return false;
    }
    const maybeError = error as { code?: number };
    const code = maybeError.code;
    return code === 201 || code === 202;
  }
}
