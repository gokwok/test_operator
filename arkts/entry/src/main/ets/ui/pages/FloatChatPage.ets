import type { common, Want } from '@kit.AbilityKit';
import { FloatChatInputBar } from '../views/FloatChatInputBar';
import { AgentControllerClient } from '../../features/agent_controller/ipc/AgentControllerClient';
import { asrFeature } from '../../features/voice_kit/Asr';

const SERVICE_BUNDLE_NAME: string = 'com.huawei.aios.agent.operation';
const SERVICE_ABILITY_NAME: string = 'AgentServiceExtAbility';
const ACTION_START_TASK: string = 'start_task';
type InputMode = 'voice' | 'keyboard';

@Entry
@Component
struct FloatChatPage {
  @State private inputText: string = '';
  @State private isVisible: boolean = false;
  @State @Watch('onSendSequence') private sendSequence: number = 0;
  @State @Watch('onMicSequence') private micSequence: number = 0;
  @State @Watch('onPttStartSequence') private pttStartSequence: number = 0;
  @State @Watch('onPttStopSequence') private pttStopSequence: number = 0;
  @State private pttPressed: boolean = false;
  @State private focusSequence: number = 0;
  @State private isListening: boolean = false;
  @State private inputMode: InputMode = 'voice';
  @State private asrLiveText: string = '';
  @State private isTransferring: boolean = false;
  @State private transferText: string = '';
  @State private transferProgress: number = 0;
  @StorageProp('FloatWindow_IsClosing') @Watch('onClosingChange') private isClosing: boolean = false;
  @StorageProp('FloatWindow_IsWaiting') private isWaiting: boolean = false;

  private readonly SIDE_PADDING: number = 16;
  private readonly BOTTOM_PADDING: number = 24;
  private readonly ANIMATION_DURATION_MS: number = 350;
  private readonly ENTER_DELAY_MS: number = 50;
  private readonly TRANSFER_DURATION_MS: number = 260;
  private readonly PTT_AUTO_RESTART_DELAY_MS: number = 80;
  private enterTimerId: number = -1;
  private transferTimerId: number = -1;
  private pttRestartTimerId: number = -1;
  private serviceClient: AgentControllerClient = new AgentControllerClient();
  private asrBaseText: string = '';
  private asrPartialText: string = '';
  private listeningToken: number = 0;

  aboutToAppear(): void {
    AppStorage.setOrCreate('FloatWindow_IsClosing', false);
    this.isVisible = false;
    this.inputMode = 'voice';
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
    if (this.transferTimerId !== -1) {
      clearTimeout(this.transferTimerId);
      this.transferTimerId = -1;
    }
    this.clearPttRestartTimer();
    this.stopListening();
    this.inputText = '';
    this.asrLiveText = '';
    this.resetTransfer();
  }

  private onClosingChange(): void {
    if (!this.isClosing) {
      return;
    }
    this.clearPttRestartTimer();
    this.stopListening();
    this.asrLiveText = '';
    this.resetTransfer();
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
          pttStartSequence: $pttStartSequence,
          pttStopSequence: $pttStopSequence,
          pttPressed: $pttPressed,
          focusSequence: this.focusSequence,
          inputEnabled: !this.isClosing && !this.isWaiting,
          inputMode: this.inputMode,
          isListening: this.isListening,
          // Keep showing the last recognized ASR text even if the recognizer stops early
          // (e.g., silence triggers onComplete) while the user is still holding PTT.
          asrBubbleText: this.asrLiveText.trim().length > 0 ? this.asrLiveText : this.inputText,
          isTransferring: this.isTransferring,
          transferText: this.transferText,
          transferProgress: this.transferProgress,
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
    this.clearPttRestartTimer();
    if (this.isListening) {
      this.stopListening();
    }
    if (this.inputMode === 'voice') {
      this.resetTransfer();
      this.inputMode = 'keyboard';
      this.requestInputFocus();
      return;
    }
    this.resetTransfer();
    this.inputMode = 'voice';
  }

  private requestInputFocus(): void {
    this.focusSequence += 1;
  }

  private onPttStartSequence(): void {
    if (this.isClosing || this.isWaiting || this.isListening) {
      return;
    }
    this.clearPttRestartTimer();
    this.resetTransfer();
    this.inputMode = 'voice';
    void this.startListening();
  }

  private onPttStopSequence(): void {
    this.clearPttRestartTimer();
    this.stopListening();
    const mergedText = this.asrLiveText.trim();
    if (mergedText.length > 0) {
      this.beginTransferToInput(this.asrLiveText);
    } else {
      this.asrLiveText = '';
      this.inputMode = 'keyboard';
    }
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
    const token = this.nextListeningToken();
    this.inputMode = 'voice';
    const baseText = this.asrLiveText.trim().length > 0 ? this.asrLiveText : this.inputText;
    this.asrBaseText = baseText;
    this.asrPartialText = '';
    this.asrLiveText = baseText;
    this.isListening = true;
    const started = await asrFeature.startListening(ctx, {
      onStart: () => {
        if (!this.isListeningTokenActive(token)) {
          return;
        }
        this.isListening = true;
      },
      onPartialResult: (text: string) => {
        if (!this.isListeningTokenActive(token)) {
          return;
        }
        this.updateAsrText(text);
      },
      onFinalResult: (text: string) => {
        if (!this.isListeningTokenActive(token)) {
          return;
        }
        this.updateAsrText(text);
      },
      onComplete: () => {
        if (!this.isListeningTokenActive(token)) {
          return;
        }
        this.isListening = false;
        this.clearAsrBuffer();
        this.schedulePttAutoRestart();
      },
      onError: () => {
        if (!this.isListeningTokenActive(token)) {
          return;
        }
        this.isListening = false;
        this.clearAsrBuffer();
        this.schedulePttAutoRestart();
      }
    });
    if (!started) {
      this.isListening = false;
      this.clearAsrBuffer();
      this.schedulePttAutoRestart();
    }
  }

  private stopListening(): void {
    this.clearPttRestartTimer();
    if (!this.isListening) {
      this.invalidateListeningToken();
      return;
    }
    this.invalidateListeningToken();
    this.isListening = false;
    asrFeature.stopListening(true);
  }

  private updateAsrText(text: string): void {
    // Some ASR engines may emit empty/whitespace results during silence. Treat those as
    // "no update" so we don't clear already recognized interim text while the user is
    // still holding the PTT button.
    if (text.trim().length === 0) {
      return;
    }
    this.asrPartialText = text;
    this.asrLiveText = this.mergeAsrText(this.asrBaseText, this.asrPartialText);
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

  private schedulePttAutoRestart(): void {
    if (!this.pttPressed || this.isClosing || this.isWaiting || this.inputMode !== 'voice') {
      return;
    }
    if (this.pttRestartTimerId !== -1) {
      return;
    }
    this.pttRestartTimerId = setTimeout(() => {
      this.pttRestartTimerId = -1;
      if (!this.pttPressed || this.isClosing || this.isWaiting || this.isListening || this.inputMode !== 'voice') {
        return;
      }
      void this.startListening();
    }, this.PTT_AUTO_RESTART_DELAY_MS) as number;
  }

  private clearPttRestartTimer(): void {
    if (this.pttRestartTimerId === -1) {
      return;
    }
    clearTimeout(this.pttRestartTimerId);
    this.pttRestartTimerId = -1;
  }

  private beginTransferToInput(text: string): void {
    const value = text.trimEnd();
    if (value.length === 0) {
      this.asrLiveText = '';
      this.inputMode = 'keyboard';
      return;
    }
    if (this.transferTimerId !== -1) {
      clearTimeout(this.transferTimerId);
      this.transferTimerId = -1;
    }
    this.inputText = value;
    this.transferText = value;
    this.isTransferring = true;
    this.transferProgress = 0;
    this.inputMode = 'keyboard';
    animateTo({ duration: this.TRANSFER_DURATION_MS, curve: Curve.EaseOut }, () => {
      this.transferProgress = 1;
    });
    this.transferTimerId = setTimeout(() => {
      this.transferTimerId = -1;
      this.asrLiveText = '';
      this.clearAsrBuffer();
      this.resetTransfer();
    }, this.TRANSFER_DURATION_MS) as number;
  }

  private resetTransfer(): void {
    if (this.transferTimerId !== -1) {
      clearTimeout(this.transferTimerId);
      this.transferTimerId = -1;
    }
    this.isTransferring = false;
    this.transferText = '';
    this.transferProgress = 0;
  }

  private nextListeningToken(): number {
    this.listeningToken += 1;
    return this.listeningToken;
  }

  private invalidateListeningToken(): void {
    this.listeningToken += 1;
  }

  private isListeningTokenActive(token: number): boolean {
    return token === this.listeningToken;
  }
}
