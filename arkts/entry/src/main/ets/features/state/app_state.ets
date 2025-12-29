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

  private static buildItem(info: bundleManager.BundleInfo): AppListItem {
    return {
      bundleName: info.name,
      versionName: info.versionName,
      versionCode: Number(info.versionCode),
    };
  }

}
