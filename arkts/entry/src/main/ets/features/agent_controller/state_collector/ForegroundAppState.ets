import appManager from '@ohos.app.ability.appManager';

export class ForegroundAppState {
  static async getForegroundAppBundleName(): Promise<string> {
    try {
      const apps: appManager.AppStateData[] = await appManager.getForegroundApplications();
      if (apps.length === 0) {
        return '';
      }
      const selected = ForegroundAppState.selectForegroundApp(apps);
      return selected ? selected.bundleName : '';
    } catch (_e) {
      return '';
    }
  }

  private static selectForegroundApp(apps: appManager.AppStateData[]): appManager.AppStateData | null {
    let foreground: appManager.AppStateData | null = null;
    for (let i = 0; i < apps.length; i++) {
      const current = apps[i];
      if (!foreground) {
        foreground = current;
        continue;
      }
      if (current.state === appManager.ApplicationState.STATE_ACTIVE) {
        foreground = current;
        break;
      }
      if (foreground.state !== appManager.ApplicationState.STATE_ACTIVE &&
        current.state === appManager.ApplicationState.STATE_FOREGROUND) {
        foreground = current;
      }
    }
    return foreground;
  }
}
