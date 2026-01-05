import { FadeVisibility } from './FadeVisibility';

type ActionType = 'supplement' | 'takeover' | 'stop';
export type StatusFloatMode = 'default' | 'call_user_wait' | 'call_user_input' | 'take_over_wait';
export type StatusFloatTerminalKind = 'stopped' | 'finished';
type SystemSymbolName = 'chevron_down' | 'chevron_up_circle' | 'mic_fill';

@Observed
export class StatusFloatWindowController {
  statusText: string = '';
  stopped: boolean = false;
  terminalKind: StatusFloatTerminalKind = 'stopped';
  terminalContent: string = '';
  mode: StatusFloatMode = 'default';
  callUserContent: string = '';
  callUserInput: string = '';
  callUserActionLabel: string = '告诉AIOS';
  callUserOptions: Array<string> = [];

  private supplementHandler: () => void = () => {};
  private takeoverHandler: () => void = () => {};
  private stopHandler: () => void = () => {};
  private destroyHandler: () => void = () => {};
  private callUserConfirmHandler: () => void = () => {};
  private callUserBackHandler: () => void = () => {};
  private callUserSendHandler: (text: string) => void = () => {};
  private callUserOptionHandler: (label: string) => void = () => {};

  static create(
    statusText?: string,
    supplement?: () => void,
    takeover?: () => void,
    stop?: () => void,
    destroy?: () => void
  ): StatusFloatWindowController {
    const controller = new StatusFloatWindowController();
    if (statusText) {
      controller.statusText = statusText;
    }
    if (supplement) {
      controller.supplementHandler = supplement;
    }
    if (takeover) {
      controller.takeoverHandler = takeover;
    }
    if (stop) {
      controller.stopHandler = stop;
    }
    if (destroy) {
      controller.destroyHandler = destroy;
    }
    return controller;
  }

  setStatus(text: string): void {
    this.statusText = text;
  }

  setMode(mode: StatusFloatMode): void {
    this.mode = mode;
  }

  setStopped(stopped: boolean): void {
    this.stopped = stopped;
    if (!stopped) {
      this.terminalKind = 'stopped';
      this.terminalContent = '';
    }
  }

  setTerminal(kind: StatusFloatTerminalKind, content: string): void {
    this.terminalKind = kind;
    this.terminalContent = content;
    this.stopped = true;
  }

  setSupplementHandler(handler: () => void): void {
    this.supplementHandler = handler;
  }

  setTakeoverHandler(handler: () => void): void {
    this.takeoverHandler = handler;
  }

  setStopHandler(handler: () => void): void {
    this.stopHandler = handler;
  }

  setDestroyHandler(handler: () => void): void {
    this.destroyHandler = handler;
  }

  setCallUserContent(content: string): void {
    this.callUserContent = content;
  }

  setCallUserInput(text: string): void {
    this.callUserInput = text;
  }

  clearCallUserInput(): void {
    this.callUserInput = '';
  }

  setCallUserActionLabel(label: string): void {
    this.callUserActionLabel = label;
  }

  setCallUserOptions(options: Array<string>): void {
    this.callUserOptions = options;
  }

  setCallUserConfirmHandler(handler: () => void): void {
    this.callUserConfirmHandler = handler;
  }

  setCallUserBackHandler(handler: () => void): void {
    this.callUserBackHandler = handler;
  }

  setCallUserSendHandler(handler: (text: string) => void): void {
    this.callUserSendHandler = handler;
  }

  setCallUserOptionHandler(handler: (label: string) => void): void {
    this.callUserOptionHandler = handler;
  }

  triggerSupplement(): void {
    this.supplementHandler();
  }

  triggerTakeover(): void {
    this.takeoverHandler();
  }

  triggerStop(): void {
    this.stopHandler();
    this.stopped = true;
    this.terminalKind = 'stopped';
    this.terminalContent = '';
  }

  triggerDestroy(): void {
    this.destroyHandler();
  }

  triggerCallUserConfirm(): void {
    this.callUserConfirmHandler();
  }

  triggerCallUserBack(): void {
    this.callUserBackHandler();
  }

  triggerCallUserSend(): void {
    const text = this.callUserInput.trim();
    if (text.length === 0) {
      return;
    }
    this.callUserSendHandler(text);
  }

  triggerCallUserOption(label: string): void {
    this.callUserOptionHandler(label);
  }
}

@Component
export struct StatusFloatWindow {
  @ObjectLink controller: StatusFloatWindowController;
  @Prop visible: boolean = true;
  @State private callUserPlaceholderText: string = '发消息或按住说话…';

  private readonly stopTitle: string = '用户已停止任务执行';
  private readonly stopDescription: string = '用户主动停止了当前任务的执行。';
  private readonly finishTitle: string = '任务已完成';
  private readonly collapseLabel: string = '收起';
  private readonly callUserTitle: string = '等待答复';
  private readonly takeOverTitle: string = '请手动操作';
  private readonly callUserHeader: string = '操作智能体';
  private readonly callUserPlaceholder: string = '发消息或按住说话…';
  private readonly glassSurfaceColor: string = '#66FFFFFF';
  private readonly glassSurfaceStrongColor: string = '#80FFFFFF';
  private readonly glassButtonColor: string = '#52FFFFFF';
  private readonly glassBorderColor: string = '#80FFFFFF';
  private readonly glassShadowColor: string = '#1F000000';
  private readonly glassShadowSoftColor: string = '#14000000';
  private readonly glassBlurRadius: number = 22;
  private readonly glassButtonBlurRadius: number = 14;
  private readonly glassCollapseBlurRadius: number = 16;
  private readonly textPrimaryColor: string = '#2B2D35';
  private readonly textSecondaryColor: string = '#8D9098';
  private readonly accentColor: string = '#FF4D5B';
  private readonly successColor: string = '#22C55E';

  private getActionColor(action: ActionType): string {
    return action === 'stop' ? this.accentColor : this.textPrimaryColor;
  }

  @Builder
  private actionButton(label: string, action: ActionType, onTap: () => void) {
    Row({ space: 4 }) {
      this.actionIcon(action, this.getActionColor(action));
      Text(label)
        .fontSize(14)
        .fontColor(this.getActionColor(action))
        .fontWeight(FontWeight.Medium)
    }
    .padding({ left: 12, right: 12, top: 6, bottom: 6 })
    .borderRadius(16)
    .backdropBlur(this.glassButtonBlurRadius)
    .backgroundColor(this.glassButtonColor)
    .borderWidth(1)
    .borderColor(this.glassBorderColor)
    .shadow({ radius: 10, color: this.glassShadowSoftColor, offsetX: 0, offsetY: 4 })
    .onClick(onTap)
  }

  @Builder
  private actionIcon(action: ActionType, color: string) {
    Stack() {
      if (action === 'supplement') {
        Row()
          .width(10)
          .height(2)
          .backgroundColor(color)
          .borderRadius(1)
        Column()
          .width(2)
          .height(10)
          .backgroundColor(color)
          .borderRadius(1)
      } else if (action === 'takeover') {
        Column({ space: 1 }) {
          Row({ space: 1 }) {
            Row()
              .width(2)
              .height(6)
              .backgroundColor(color)
              .borderRadius(1)
            Row()
              .width(2)
              .height(6)
              .backgroundColor(color)
              .borderRadius(1)
            Row()
              .width(2)
              .height(6)
              .backgroundColor(color)
              .borderRadius(1)
          }
          Row()
            .width(8)
            .height(4)
            .backgroundColor(color)
            .borderRadius(2)
        }
      } else {
        Row()
          .width(8)
          .height(8)
          .backgroundColor(color)
          .borderRadius(2)
      }
    }
    .width(22)
    .height(22)
  }

  @Builder
  private stoppedContent() {
    Column({ space: 10 }) {
      Column({ space: 6 }) {
        Text(this.controller.terminalKind === 'finished' ? this.finishTitle : this.stopTitle)
          .fontSize(14)
          .fontColor(this.controller.terminalKind === 'finished' ? this.successColor : this.accentColor)
          .fontWeight(FontWeight.Bold)
        Text(this.controller.terminalContent.trim().length > 0
          ? this.controller.terminalContent.trim()
          : (this.controller.terminalKind === 'finished' ? '' : this.stopDescription))
          .fontSize(12)
          .fontColor(this.textSecondaryColor)
      }
      .width('100%')
      .alignItems(HorizontalAlign.Start)
      .padding({ left: 18, right: 18, top: 22, bottom: 20 })
    }
  }

  @Builder
  private activeContent() {
    Column({ space: 16 }) {
      Column() {
        Text(this.controller.statusText)
          .fontSize(14)
          .fontColor(this.accentColor)
          .fontWeight(FontWeight.Bold)
          .textAlign(TextAlign.Start)
          .width('100%')
      }
      .width('100%')
      .alignItems(HorizontalAlign.Start)
      .padding({ left: 16, right: 16, top: 18 })

      Row() {
        Row({ space: 10 }) {
          this.actionButton('补充', 'supplement', () => this.controller.triggerSupplement());
          this.actionButton('接管', 'takeover', () => this.controller.triggerTakeover());
        }
        this.actionButton('停止', 'stop', () => this.controller.triggerStop());
      }
      .width('100%')
      .justifyContent(FlexAlign.SpaceBetween)
      .padding({ left: 8, right: 8, bottom: 16 })
    }
  }

  @Builder
  private callUserWaitContent() {
    Column({ space: 16 }) {
      Column({ space: 8 }) {
        this.callUserBubble();
        Text(this.resolveWaitTitle())
          .fontSize(13)
          .fontColor(this.textPrimaryColor)
          .textAlign(TextAlign.Center)
          .width('100%')
      }
      .width('100%')
      .alignItems(HorizontalAlign.Center)

      Text(this.controller.callUserContent)
        .fontSize(14)
        .fontColor(this.accentColor)
        .fontWeight(FontWeight.Bold)
        .textAlign(TextAlign.Start)
        .width('100%')

      Column({ space: 6 }) {
        ForEach(this.resolveCallUserOptions(), (label: string) => {
          Button(label)
            .fontSize(15)
            .fontColor(this.accentColor)
            .fontWeight(FontWeight.Medium)
            .backgroundColor(this.glassButtonColor)
            .height(40)
            .borderRadius(14)
            .backdropBlur(this.glassButtonBlurRadius)
            .borderWidth(1)
            .borderColor(this.glassBorderColor)
            .shadow({ radius: 10, color: this.glassShadowSoftColor, offsetX: 0, offsetY: 4 })
            .width('100%')
            .onClick(() => this.controller.triggerCallUserOption(label))
        }, (label: string) => label)
      }
      .width('100%')

      Row() {
        Row()
          .layoutWeight(1)
        this.actionButton('停止', 'stop', () => this.controller.triggerStop())
      }
      .width('100%')
    }
    .width('100%')
    .padding({ left: 18, right: 18, top: 20, bottom: 16 })
    .backdropBlur(this.glassBlurRadius)
    .backgroundColor(this.glassSurfaceColor)
    .borderRadius(26)
    .borderWidth(1)
    .borderColor(this.glassBorderColor)
    .shadow({ radius: 18, color: this.glassShadowColor, offsetX: 0, offsetY: 10 })
    .clip(true)
  }

  @Builder
  private callUserInputContent() {
    Column({ space: 12 }) {
      Row({ space: 6 }) {
        SymbolGlyph($r('sys.symbol.hand_point_up_tap_fill_1'))
          .fontSize(16)
          .fontColor([this.textPrimaryColor])
        Text(this.callUserHeader)
          .fontSize(13)
          .fontColor(this.textPrimaryColor)
        Row()
          .layoutWeight(1)
        this.iconButton('chevron_down', () => this.controller.triggerCallUserBack())
      }
      .width('100%')
      .padding({ left: 16, right: 16, top: 14 })

      Text(this.controller.callUserContent)
        .fontSize(14)
        .fontColor(this.accentColor)
        .fontWeight(FontWeight.Bold)
        .lineHeight(20)
        .width('100%')
        .padding({ left: 16, right: 16 })

      Row()
        .width('100%')
        .height(1)
        .backgroundColor('#E6E8EB')

      Row({ space: 10 }) {
        TextInput({ text: this.controller.callUserInput, placeholder: this.callUserPlaceholderText })
          .layoutWeight(1)
          .height(36)
          .fontSize(13)
          .fontColor(this.textPrimaryColor)
          .backgroundColor('#00000000')
          .borderRadius(12)
          .padding({ left: 6, right: 6, top: 2, bottom: 2 })
          .placeholderColor('#B6BAC4')
          .onChange((value: string) => {
            this.controller.setCallUserInput(value);
          })
          .onFocus(() => {
            this.callUserPlaceholderText = '';
          })
          .onBlur(() => {
            if (this.controller.callUserInput.trim().length === 0) {
              this.callUserPlaceholderText = this.callUserPlaceholder;
            }
          })

        if (this.controller.callUserInput.trim().length > 0) {
          this.iconButton('chevron_up_circle', () => this.controller.triggerCallUserSend())
        } else {
          this.iconButton('mic_fill', () => {})
        }
      }
      .width('100%')
      .padding({ left: 16, right: 16, bottom: 16 })
    }
    .width('100%')
    .backdropBlur(this.glassBlurRadius)
    .backgroundColor(this.glassSurfaceColor)
    .borderRadius(22)
    .borderWidth(1)
    .borderColor(this.glassBorderColor)
    .shadow({ radius: 18, color: this.glassShadowColor, offsetX: 0, offsetY: 8 })
    .clip(true)
  }

  private resolveCallUserOptions(): Array<string> {
    if (this.controller.callUserOptions.length > 0) {
      return this.controller.callUserOptions;
    }
    if (this.controller.callUserActionLabel.length > 0) {
      return [this.controller.callUserActionLabel];
    }
    return [];
  }

  private resolveWaitTitle(): string {
    if (this.controller.mode === 'take_over_wait') {
      return this.takeOverTitle;
    }
    return this.callUserTitle;
  }

  @Builder
  private iconButton(symbolName: SystemSymbolName, onTap: () => void) {
    Row() {
      this.symbolGlyph(symbolName);
    }
    .width(28)
    .height(28)
    .borderRadius(14)
    .backdropBlur(this.glassButtonBlurRadius)
    .backgroundColor(this.glassButtonColor)
    .borderWidth(1)
    .borderColor(this.glassBorderColor)
    .justifyContent(FlexAlign.Center)
    .alignItems(VerticalAlign.Center)
    .onClick(onTap)
  }

  @Builder
  private symbolGlyph(symbolName: SystemSymbolName) {
    if (symbolName === 'chevron_down') {
      SymbolGlyph($r('sys.symbol.chevron_down'))
        .fontSize(18)
        .fontColor([this.textPrimaryColor])
    } else if (symbolName === 'mic_fill') {
      SymbolGlyph($r('sys.symbol.mic_fill'))
        .fontSize(16)
        .fontColor([this.textPrimaryColor])
    } else {
      SymbolGlyph($r('sys.symbol.chevron_up_circle'))
        .fontSize(20)
        .fontColor([this.textPrimaryColor])
    }
  }

  @Builder
  private callUserBubble() {
    Row({ space: 4 }) {
      Row()
        .width(6)
        .height(6)
        .borderRadius(3)
        .backgroundColor('#FF9AA2')
      Row()
        .width(6)
        .height(6)
        .borderRadius(3)
        .backgroundColor('#FF9AA2')
      Row()
        .width(6)
        .height(6)
        .borderRadius(3)
        .backgroundColor('#FF9AA2')
    }
    .width(36)
    .height(36)
    .borderRadius(18)
    .backgroundColor('#FFE3E6')
    .justifyContent(FlexAlign.Center)
    .alignItems(VerticalAlign.Center)
  }

  @Builder
  private collapseButton() {
    Row({ space: 6 }) {
      Image($r('app.media.ic_public_close_filled'))
        .width(14)
        .height(14)
      Text(this.collapseLabel)
        .fontSize(13)
        .fontColor(this.textPrimaryColor)
        .fontWeight(FontWeight.Bold)
    }
    .padding({ left: 16, right: 16, top: 9, bottom: 9 })
    .borderRadius(18)
    .backdropBlur(this.glassCollapseBlurRadius)
    .backgroundColor(this.glassSurfaceStrongColor)
    .borderWidth(1)
    .borderColor(this.glassBorderColor)
    .shadow({ radius: 12, color: this.glassShadowSoftColor, offsetX: 0, offsetY: 6 })
    .onClick(() => this.controller.triggerDestroy())
  }

  @Builder
  private windowContent() {
    Column({ space: 10 }) {
      if (this.controller.stopped) {
        Column() {
          this.stoppedContent();
        }
        .backdropBlur(this.glassBlurRadius)
        .backgroundColor(this.glassSurfaceColor)
        .borderRadius(26)
        .borderWidth(1)
        .borderColor(this.glassBorderColor)
        .shadow({ radius: 18, color: this.glassShadowColor, offsetX: 0, offsetY: 8 })
        .clip(true)

        this.collapseButton();
      } else if (this.controller.mode === 'call_user_wait' || this.controller.mode === 'take_over_wait') {
        this.callUserWaitContent();
      } else if (this.controller.mode === 'call_user_input') {
        this.callUserInputContent();
      } else {
        Column() {
          this.activeContent();
        }
        .backdropBlur(this.glassBlurRadius)
        .backgroundColor(this.glassSurfaceColor)
        .borderRadius(26)
        .borderWidth(1)
        .borderColor(this.glassBorderColor)
        .shadow({ radius: 18, color: this.glassShadowColor, offsetX: 0, offsetY: 8 })
        .clip(true)
      }
    }
    .alignItems(HorizontalAlign.Center)
  }

  build() {
    FadeVisibility({ visible: this.visible, durationMs: 240 }) {
      this.windowContent();
    }
  }
}
