import type { common, Want } from '@kit.AbilityKit';
import { FloatChatInputBar } from '../view/FloatChatInputBar';
import { AgentServiceClient } from '../features/agent_service/ipc/AgentServiceClient';
import { asrFeature } from '../features/Asr';

const SERVICE_BUNDLE_NAME: string = 'com.huawei.aios.agent.operation';
const SERVICE_ABILITY_NAME: string = 'AgentServiceExtAbility';
const ACTION_START_TASK: string = 'start_task';

@Entry
@Component
struct FloatChatPage {
  @State private inputText: string = '';
  @State private isVisible: boolean = false;
  @State @Watch('onSendSequence') private sendSequence: number = 0;
  @State @Watch('onMicSequence') private micSequence: number = 0;
  @State private isListening: boolean = false;
  @StorageProp('FloatWindow_IsClosing') @Watch('onClosingChange') private isClosing: boolean = false;
  @StorageProp('FloatWindow_IsWaiting') private isWaiting: boolean = false;

  private readonly SIDE_PADDING: number = 16;
  private readonly BOTTOM_PADDING: number = 24;
  private readonly ANIMATION_DURATION_MS: number = 350;
  private readonly ENTER_DELAY_MS: number = 50;
  private enterTimerId: number = -1;
  private serviceClient: AgentServiceClient = new AgentServiceClient();
  private asrBaseText: string = '';
  private asrPartialText: string = '';

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
    this.stopListening();
    this.inputText = '';
  }

  private onClosingChange(): void {
    if (!this.isClosing) {
      return;
    }
    this.stopListening();
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
          micSequence: $micSequence,
          inputEnabled: !this.isClosing && !this.isWaiting,
          isListening: this.isListening,
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

  private onMicSequence(): void {
    if (this.isClosing || this.isWaiting) {
      return;
    }
    if (this.isListening) {
      this.stopListening();
      return;
    }
    void this.startListening();
  }

  private async dispatchTask(): Promise<void> {
    const taskText = this.inputText.trim();
    if (taskText.length === 0) {
      return;
    }
    AppStorage.setOrCreate('FloatWindow_IsWaiting', true);
    try {
      const ctx = getContext(this) as common.UIExtensionContext;
      await this.ensureServiceStarted(ctx);
      await this.connectService(ctx);
      await this.serviceClient.startTaskText(taskText);
    } catch (e) {
      console.error(`[FloatChatPage] dispatch task failed: ${String(e)}`);
      await this.dispatchTaskFallback(taskText);
    } finally {
      AppStorage.setOrCreate('FloatWindow_IsWaiting', false);
    }
  }

  private async connectService(ctx: common.UIExtensionContext): Promise<void> {
    await this.serviceClient.connectFromExtension(ctx);
  }

  private async ensureServiceStarted(ctx: common.UIExtensionContext): Promise<void> {
    const want: Want = {
      bundleName: SERVICE_BUNDLE_NAME,
      abilityName: SERVICE_ABILITY_NAME,
      action: 'keep_alive',
    };
    try {
      await ctx.startServiceExtensionAbility(want);
    } catch (e) {
      console.warn(`[FloatChatPage] keep_alive start failed: ${String(e)}`);
    }
  }

  private async dispatchTaskFallback(taskText: string): Promise<void> {
    try {
      const ctx = getContext(this) as common.UIExtensionContext;
      const encodedTask = encodeURIComponent(taskText);
      const want: Want = {
        bundleName: SERVICE_BUNDLE_NAME,
        abilityName: SERVICE_ABILITY_NAME,
        action: ACTION_START_TASK,
        uri: encodedTask,
      };
      try {
        await ctx.startServiceExtensionAbility(want);
        return;
      } catch (_e) {
        await ctx.startAbility(want);
      }
    } catch (e) {
      console.error(`[FloatChatPage] fallback start_task failed: ${String(e)}`);
    }
  }

  private async startListening(): Promise<void> {
    const ctx = getContext(this) as common.UIAbilityContext;
    this.asrBaseText = this.inputText;
    this.asrPartialText = '';
    this.isListening = true;
    const started = await asrFeature.startListening(ctx, {
      onStart: () => {
        this.isListening = true;
      },
      onPartialResult: (text: string) => {
        this.updateAsrText(text);
      },
      onFinalResult: (text: string) => {
        this.updateAsrText(text);
      },
      onComplete: () => {
        this.isListening = false;
        this.clearAsrBuffer();
      },
      onError: () => {
        this.isListening = false;
        this.clearAsrBuffer();
      }
    });
    if (!started) {
      this.isListening = false;
      this.clearAsrBuffer();
    }
  }

  private stopListening(): void {
    if (!this.isListening) {
      return;
    }
    this.isListening = false;
    asrFeature.stopListening(true);
  }

  private updateAsrText(text: string): void {
    this.asrPartialText = text;
    this.inputText = this.mergeAsrText(this.asrBaseText, this.asrPartialText);
  }

  private mergeAsrText(baseText: string, asrText: string): string {
    if (asrText.length === 0) {
      return baseText;
    }
    if (baseText.length === 0) {
      return asrText;
    }
    const baseChar = baseText.charAt(baseText.length - 1);
    const asrChar = asrText.charAt(0);
    const needsSpace = /[A-Za-z0-9]/.test(baseChar) && /[A-Za-z0-9]/.test(asrChar);
    return needsSpace ? `${baseText} ${asrText}` : `${baseText}${asrText}`;
  }

  private clearAsrBuffer(): void {
    this.asrBaseText = '';
    this.asrPartialText = '';
  }
}
