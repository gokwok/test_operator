import { AbilityConstant, UIAbility, Want } from '@kit.AbilityKit';
import type { BusinessError } from '@kit.BasicServicesKit';
import { hilog } from '@kit.PerformanceAnalysisKit';
import { window } from '@kit.ArkUI';

const DOMAIN = 0x0000;
const TAG = 'ScreenboardAbility';

export default class ScreenboardAbility extends UIAbility {
  private currentWindowStage: window.WindowStage | null = null;

  onCreate(want: Want, launchParam: AbilityConstant.LaunchParam): void {
    void want;
    void launchParam;
    hilog.info(DOMAIN, TAG, 'onCreate');
  }

  onWindowStageCreate(windowStage: window.WindowStage): void {
    hilog.info(DOMAIN, TAG, 'onWindowStageCreate');
    const windowClass: window.Window = windowStage.getMainWindowSync();
    windowClass.setWindowLayoutFullScreen(true).catch((err: BusinessError) => {
      hilog.error(DOMAIN, TAG, 'setWindowLayoutFullScreen failed. Code:%{public}d, msg:%{public}s', err.code, err.message);
    });

    if (this.currentWindowStage === null) {
      this.currentWindowStage = windowStage;
    }
    windowStage.loadContent('ui/pages/ScreenboardPage', (err) => {
      if (err.code) {
        hilog.error(DOMAIN, TAG, 'loadContent failed. err=%{public}s', JSON.stringify(err));
      }
    });
  }
}
