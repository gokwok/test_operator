import bundleManager from '@ohos.bundle.bundleManager';

export interface AppListItem {
  bundleName: string;
  versionName: string;
  versionCode: number;
}

export class AppState {
  static async listInstalledApps(): Promise<AppListItem[]> {
    const flags: number = bundleManager.BundleFlag.GET_BUNDLE_INFO_DEFAULT;
    const bundles: Array<bundleManager.BundleInfo> = await bundleManager.getAllBundleInfo(flags);
    const items: AppListItem[] = [];
    for (let i = 0; i < bundles.length; i++) {
      const info: bundleManager.BundleInfo = bundles[i];
      const bundleName: string = info.name;
      if (bundleName.length === 0) {
        continue;
      }
      items.push({
        bundleName,
        versionName: info.versionName,
        versionCode: Number(info.versionCode),
      });
    }
    items.sort((a, b) => a.bundleName.localeCompare(b.bundleName));
    return items;
  }
}
