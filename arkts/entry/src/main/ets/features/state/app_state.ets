import bundleManager from '@ohos.bundle.bundleManager';

export interface AppListItem {
  bundleName: string;
  versionName: string;
  versionCode: number;
}

export class AppState {
  static async listInstalledApps(): Promise<AppListItem[]> {
    const bundles = await bundleManager.getBundleInfos(
      bundleManager.BundleFlag.GET_BUNDLE_INFO_WITH_APPLICATION
    );
    const items: AppListItem[] = [];
    for (let i = 0; i < bundles.length; i++) {
      const info = bundles[i];
      const bundleName = info.bundleName ?? '';
      if (bundleName.length === 0) {
        continue;
      }
      items.push({
        bundleName,
        versionName: info.versionName ?? '',
        versionCode: Number(info.versionCode ?? 0),
      });
    }
    items.sort((a, b) => a.bundleName.localeCompare(b.bundleName));
    return items;
  }
}
