import type { common, Want } from '@kit.AbilityKit';
import { FloatChatInputBar } from '../ui_components/FloatChatInputBar';
import { AgentServiceClient } from '../features/agent_service/ipc/AgentServiceClient';

const SERVICE_BUNDLE_NAME: string = 'com.huawei.aios.agent.operation';
const SERVICE_ABILITY_NAME: string = 'AgentServiceExtAbility';
const ACTION_START_TASK: string = 'start_task';

interface ServiceExtensionStarter {
  startServiceExtensionAbility?: (w: Want) => Promise<void> | void;
}

interface AbilityStarter {
  startAbility?: (w: Want) => Promise<void> | void;
}

interface ServiceConnectContext {
  connectServiceExtensionAbility(want: Want, options: common.ConnectOptions): number;
}

@Entry
@Component
struct FloatChatPage {
  @State private inputText: string = '';
  @State private isVisible: boolean = false;
  @State @Watch('onSendSequence') private sendSequence: number = 0;
  @StorageProp('FloatWindow_IsClosing') @Watch('onClosingChange') private isClosing: boolean = false;
  @StorageProp('FloatWindow_IsWaiting') private isWaiting: boolean = false;

  private readonly SIDE_PADDING: number = 16;
  private readonly BOTTOM_PADDING: number = 24;
  private readonly ANIMATION_DURATION_MS: number = 350;
  private readonly ENTER_DELAY_MS: number = 50;
  private enterTimerId: number = -1;
  private serviceClient: AgentServiceClient = new AgentServiceClient();

  aboutToAppear(): void {
    AppStorage.setOrCreate('FloatWindow_IsClosing', false);
    this.isVisible = false;
    if (this.enterTimerId !== -1) {
      clearTimeout(this.enterTimerId);
      this.enterTimerId = -1;
    }
    this.enterTimerId = setTimeout(() => {
      animateTo({ duration: this.ANIMATION_DURATION_MS, curve: Curve.EaseOut }, () => {
        this.isVisible = true;
      });
      this.enterTimerId = -1;
    }, this.ENTER_DELAY_MS) as number;
  }

  aboutToDisappear(): void {
    if (this.enterTimerId !== -1) {
      clearTimeout(this.enterTimerId);
      this.enterTimerId = -1;
    }
  }

  private onClosingChange(): void {
    if (!this.isClosing) {
      return;
    }
    animateTo({ duration: this.ANIMATION_DURATION_MS, curve: Curve.EaseOut }, () => {
      this.isVisible = false;
    });
  }

  build() {
    Stack() {
      Column() {
        Blank()

        FloatChatInputBar({
          text: $inputText,
          sendSequence: $sendSequence,
          inputEnabled: !this.isClosing && !this.isWaiting,
        })
          .opacity(this.isVisible ? 1 : 0)
          .translate({ y: this.isVisible ? 0 : 20 })
      }
      .width('100%')
      .height('100%')
      .justifyContent(FlexAlign.End)
      .padding({ left: this.SIDE_PADDING, right: this.SIDE_PADDING, bottom: this.BOTTOM_PADDING })
    }
    .width('100%')
    .height('100%')
    .backgroundColor('rgba(0, 0, 0, 0)')
    .hitTestBehavior(HitTestMode.Transparent)
  }

  private onSendSequence(): void {
    void this.dispatchTask();
  }

  private async dispatchTask(): Promise<void> {
    const taskText = this.inputText.trim();
    if (taskText.length === 0) {
      return;
    }
    AppStorage.setOrCreate('FloatWindow_IsWaiting', true);
    try {
      const ctx = getContext(this) as common.Context;
      await this.connectService(ctx);
      await this.serviceClient.startTaskText(taskText);
      this.inputText = '';
    } catch (e) {
      console.error(`[FloatChatPage] dispatch task failed: ${String(e)}`);
      await this.dispatchTaskFallback(taskText);
    } finally {
      AppStorage.setOrCreate('FloatWindow_IsWaiting', false);
    }
  }

  private async connectService(ctx: common.Context): Promise<void> {
    const connector = ctx as ServiceConnectContext;
    if (typeof connector.connectServiceExtensionAbility === 'function') {
      await this.serviceClient.connect(connector);
      return;
    }
    throw new Error('connectServiceExtensionAbility_missing');
  }

  private async dispatchTaskFallback(taskText: string): Promise<void> {
    try {
      const ctx = getContext(this) as common.Context;
      const encodedTask = encodeURIComponent(taskText);
      const want: Want = {
        bundleName: SERVICE_BUNDLE_NAME,
        abilityName: SERVICE_ABILITY_NAME,
        action: ACTION_START_TASK,
        uri: encodedTask,
      };
      const starter = (ctx as ServiceExtensionStarter).startServiceExtensionAbility;
      if (typeof starter === 'function') {
        await (ctx as ServiceExtensionStarter).startServiceExtensionAbility!(want);
        return;
      }
      const abilityStarter = (ctx as AbilityStarter).startAbility;
      if (typeof abilityStarter === 'function') {
        await (ctx as AbilityStarter).startAbility!(want);
      }
    } catch (e) {
      console.error(`[FloatChatPage] fallback start_task failed: ${String(e)}`);
    }
  }
}
