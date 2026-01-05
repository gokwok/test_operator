import ServiceExtensionAbility from '@ohos.app.ability.ServiceExtensionAbility';
import type Want from '@ohos.app.ability.Want';
import type { BusinessError } from '@kit.BasicServicesKit';
import { display, window } from '@kit.ArkUI';
import { hilog } from '@kit.PerformanceAnalysisKit';

const TAG = 'FloatWindowAbility';
const DOMAIN = 0x0000;

interface FloatWindowParams {
  action?: string;
}

export default class FloatWindowAbility extends ServiceExtensionAbility {
  private floatWindow: window.Window | null = null;
  private windowName: string = 'agent_float_window';
  private readonly REGION_HEIGHT_RATIO: number = 0.15;

  onCreate(want: Want): void {
    void want;
    hilog.info(DOMAIN, TAG, 'FloatWindowAbility onCreate');
  }

  async onRequest(want: Want, startId: number): Promise<void> {
    hilog.info(DOMAIN, TAG, 'FloatWindowAbility onRequest, startId: %{public}d', startId);
    const params = want.parameters as FloatWindowParams;
    const action = typeof want.action === 'string' && want.action.length > 0
      ? want.action
      : (typeof params?.action === 'string' ? params.action : '');
    switch (action) {
      case 'show':
        await this.showFloatWindow();
        break;
      case 'hide':
        await this.hideFloatWindow();
        break;
      case 'toggle':
        if (this.floatWindow) {
          await this.hideFloatWindow();
        } else {
          await this.showFloatWindow();
        }
        break;
      default:
        await this.showFloatWindow();
        break;
    }
  }

  private async showFloatWindow(): Promise<void> {
    if (this.floatWindow) {
      hilog.warn(DOMAIN, TAG, 'Float window already exists');
      return;
    }

    AppStorage.setOrCreate('FloatWindow_IsClosing', false);
    AppStorage.setOrCreate('FloatWindow_IsWaiting', false);

    try {
      let defaultDisplay: display.Display;
      try {
        defaultDisplay = display.getDefaultDisplaySync();
      } catch (e) {
        const err = e as BusinessError;
        hilog.error(
          DOMAIN,
          TAG,
          'Failed to get default display. Code=%{public}d, message=%{public}s',
          err.code,
          err.message
        );
        return;
      }

      const config: window.Configuration = {
        name: this.windowName,
        windowType: window.WindowType.TYPE_FLOAT,
        ctx: this.context
      };

      this.floatWindow = await window.createWindow(config);
      hilog.info(DOMAIN, TAG, 'Float window created');

      const regionHeight = Math.floor(defaultDisplay.height * this.REGION_HEIGHT_RATIO);
      const x = 0;
      const y = Math.max(0, defaultDisplay.height - regionHeight);

      await this.floatWindow.resize(defaultDisplay.width, regionHeight);
      await this.floatWindow.moveWindowTo(x, y);
      await this.floatWindow.setUIContent('ui/pages/FloatChatPage');
      await this.floatWindow.setWindowBackgroundColor('#00000000');

      this.floatWindow.setWindowTouchable(true);
      this.floatWindow.setWindowFocusable(true);

      this.floatWindow.on('windowEvent', (eventType: window.WindowEventType) => {
        if (eventType === window.WindowEventType.WINDOW_INACTIVE) {
          hilog.info(DOMAIN, TAG, 'Float window lost focus, closing');
          void this.hideFloatWindow();
        }
      });

      await this.floatWindow.showWindow();
      hilog.info(DOMAIN, TAG, 'Float window shown');
    } catch (error) {
      hilog.error(DOMAIN, TAG, 'Failed to create float window: %{public}s', JSON.stringify(error));
    }
  }

  private async hideFloatWindow(): Promise<void> {
    if (!this.floatWindow) {
      hilog.warn(DOMAIN, TAG, 'No float window to hide');
      return;
    }

    AppStorage.setOrCreate('FloatWindow_IsClosing', true);
    AppStorage.setOrCreate('FloatWindow_IsWaiting', false);

    setTimeout(async () => {
      if (!this.floatWindow) {
        return;
      }
      try {
        await this.floatWindow.destroyWindow();
        this.floatWindow = null;
        hilog.info(DOMAIN, TAG, 'Float window destroyed');
      } catch (error) {
        hilog.error(DOMAIN, TAG, 'Failed to destroy float window: %{public}s', JSON.stringify(error));
      }
    }, 350);
  }

  onDestroy(): void {
    hilog.info(DOMAIN, TAG, 'FloatWindowAbility onDestroy');
    if (this.floatWindow) {
      const w = this.floatWindow;
      this.floatWindow = null;
      try {
        void w.destroyWindow().catch((error: BusinessError | Error) => {
          hilog.error(DOMAIN, TAG, 'Failed to destroy float window in onDestroy: %{public}s', JSON.stringify(error));
        });
      } catch (error) {
        const err = error as BusinessError | Error;
        hilog.error(DOMAIN, TAG, 'Failed to destroy float window in onDestroy: %{public}s', JSON.stringify(err));
      }
    }
  }
}
