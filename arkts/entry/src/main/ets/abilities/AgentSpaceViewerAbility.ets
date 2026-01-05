import { AbilityConstant, UIAbility, Want } from '@kit.AbilityKit';
import { hilog } from '@kit.PerformanceAnalysisKit';
import { window } from '@kit.ArkUI';
import type { BusinessError } from '@kit.BasicServicesKit';
import { AgentControllerClient } from '../features/agent_controller/ipc/AgentControllerClient';

const DOMAIN = 0x0000;
const TAG = 'AgentSpaceViewerAbility';

export default class AgentSpaceViewerAbility extends UIAbility {
  private currentWindowStage: window.WindowStage | null = null;
  private serviceClient: AgentControllerClient | null = null;

  onCreate(want: Want, launchParam: AbilityConstant.LaunchParam): void {
    void launchParam;
    if (this.handleCloseIfNeeded(want)) {
      return;
    }
    hilog.info(DOMAIN, TAG, 'onCreate');
  }

  onNewWant(want: Want, launchParam: AbilityConstant.LaunchParam): void {
    void launchParam;
    if (this.handleCloseIfNeeded(want)) {
      return;
    }
    if (this.currentWindowStage) {
      this.onWindowStageCreate(this.currentWindowStage);
    }
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
    windowStage.loadContent('ui/pages/AgentSpaceViewer', (err) => {
      if (err.code) {
        hilog.error(DOMAIN, TAG, 'loadContent failed. err=%{public}s', JSON.stringify(err));
      }
    });
  }

  onDestroy(): void {
    void this.updateViewerActive(false);
    hilog.info(DOMAIN, TAG, 'onDestroy');
  }

  private handleCloseIfNeeded(want: Want): boolean {
    if (want?.action !== VIEWER_CLOSE_ACTION) {
      return false;
    }
    void this.updateViewerActive(false);
    this.context.terminateSelf((err: BusinessError) => {
      void err;
    });
    return true;
  }

  private getServiceClient(): AgentControllerClient {
    if (!this.serviceClient) {
      this.serviceClient = new AgentControllerClient();
    }
    return this.serviceClient;
  }

  private async updateViewerActive(active: boolean): Promise<void> {
    try {
      await this.getServiceClient().connect(this.context);
      await this.getServiceClient().setViewerActive(active);
    } catch (e) {
      hilog.warn(DOMAIN, TAG, 'updateViewerActive failed: %{public}s', String(e));
    }
  }
}

const VIEWER_CLOSE_ACTION: string = 'closeViewer';
