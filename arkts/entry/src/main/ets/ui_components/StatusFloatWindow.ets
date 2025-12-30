import { FadeVisibility } from './FadeVisibility';

type ActionType = 'supplement' | 'takeover' | 'stop';
export type StatusFloatMode = 'default' | 'call_user_wait' | 'call_user_input';
type SystemSymbolName = 'chevron_down' | 'chevron_up_circle' | 'mic_fill';

@Observed
export class StatusFloatWindowController {
  statusText: string = '';
  stopped: boolean = false;
  mode: StatusFloatMode = 'default';
  callUserContent: string = '';
  callUserInput: string = '';
  callUserActionLabel: string = '告诉AIOS';

  private supplementHandler: () => void = () => {};
  private takeoverHandler: () => void = () => {};
  private stopHandler: () => void = () => {};
  private destroyHandler: () => void = () => {};
  private callUserConfirmHandler: () => void = () => {};
  private callUserBackHandler: () => void = () => {};
  private callUserSendHandler: (text: string) => void = () => {};

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

  setCallUserConfirmHandler(handler: () => void): void {
    this.callUserConfirmHandler = handler;
  }

  setCallUserBackHandler(handler: () => void): void {
    this.callUserBackHandler = handler;
  }

  setCallUserSendHandler(handler: (text: string) => void): void {
    this.callUserSendHandler = handler;
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
}

@Component
export struct StatusFloatWindow {
  @ObjectLink controller: StatusFloatWindowController;
  @Prop visible: boolean = true;
  @State private callUserPlaceholderText: string = '发消息或按住说话…';

  private readonly stopTitle: string = '用户已停止任务执行';
  private readonly stopDescription: string = '用户主动停止了当前任务的执行。';
  private readonly collapseLabel: string = '收起';
  private readonly callUserTitle: string = '等待答复';
  private readonly callUserHeader: string = '操作智能体';
  private readonly callUserPlaceholder: string = '发消息或按住说话…';

  @Builder
  private actionButton(label: string, action: ActionType, onTap: () => void) {
    Row({ space: 4 }) {
      this.actionIcon(action);
      Text(label)
        .fontSize(14)
        .fontColor('#2B2D35')
        .fontWeight(FontWeight.Bold)
    }
    .padding({ left: 10, right: 10, top: 5, bottom: 5 })
    .borderRadius(14)
    .backgroundColor('#F3F5F7')
    .onClick(onTap)
  }

  @Builder
  private actionIcon(action: ActionType) {
    Stack() {
      if (action === 'supplement') {
        Row()
          .width(10)
          .height(2)
          .backgroundColor('#2B2D35')
          .borderRadius(1)
        Column()
          .width(2)
          .height(10)
          .backgroundColor('#2B2D35')
          .borderRadius(1)
      } else if (action === 'takeover') {
        Column({ space: 1 }) {
          Row({ space: 1 }) {
            Row()
              .width(2)
              .height(6)
              .backgroundColor('#2B2D35')
              .borderRadius(1)
            Row()
              .width(2)
              .height(6)
              .backgroundColor('#2B2D35')
              .borderRadius(1)
            Row()
              .width(2)
              .height(6)
              .backgroundColor('#2B2D35')
              .borderRadius(1)
          }
          Row()
            .width(8)
            .height(4)
            .backgroundColor('#2B2D35')
            .borderRadius(2)
        }
      } else {
        Row()
          .width(8)
          .height(8)
          .backgroundColor('#2B2D35')
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
        Text(this.stopTitle)
          .fontSize(14)
          .fontColor('#FF4D5B')
          .fontWeight(FontWeight.Bold)
        Text(this.stopDescription)
          .fontSize(12)
          .fontColor('#8D9098')
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
          .fontColor('#FF4D5B')
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
        Text(this.callUserTitle)
          .fontSize(13)
          .fontColor('#3A3D45')
          .textAlign(TextAlign.Center)
          .width('100%')
      }
      .width('100%')
      .alignItems(HorizontalAlign.Center)

      Text(this.controller.callUserContent)
        .fontSize(14)
        .fontColor('#FF4D5B')
        .fontWeight(FontWeight.Bold)
        .textAlign(TextAlign.Start)
        .width('100%')

      Button(this.controller.callUserActionLabel)
        .fontSize(15)
        .fontColor('#FF4D5B')
        .fontWeight(FontWeight.Bold)
        .backgroundColor('#FFE3E6')
        .height(40)
        .borderRadius(14)
        .width('100%')
        .onClick(() => this.controller.triggerCallUserConfirm())

      Row() {
        Row()
          .layoutWeight(1)
        this.actionButton('停止', 'stop', () => this.controller.triggerStop())
      }
      .width('100%')
    }
    .width('100%')
    .padding({ left: 18, right: 18, top: 20, bottom: 16 })
    .backgroundColor('#FFFFFF')
    .borderRadius(26)
    .shadow({ radius: 18, color: '#22000000', offsetX: 0, offsetY: 10 })
  }

  @Builder
  private callUserInputContent() {
    Column({ space: 12 }) {
      Row({ space: 6 }) {
        SymbolGlyph($r('sys.symbol.hand_point_up_tap_fill_1'))
          .fontSize(16)
          .fontColor(['#2B2D35'])
        Text(this.callUserHeader)
          .fontSize(13)
          .fontColor('#2B2D35')
        Row()
          .layoutWeight(1)
        this.iconButton('chevron_down', () => this.controller.triggerCallUserBack())
      }
      .width('100%')
      .padding({ left: 16, right: 16, top: 14 })

      Text(this.controller.callUserContent)
        .fontSize(14)
        .fontColor('#FF4D5B')
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
          .fontColor('#2B2D35')
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
    .backgroundColor('#FFFFFF')
    .borderRadius(22)
    .shadow({ radius: 18, color: '#22000000', offsetX: 0, offsetY: 8 })
  }

  @Builder
  private iconButton(symbolName: SystemSymbolName, onTap: () => void) {
    Row() {
      this.symbolGlyph(symbolName);
    }
    .width(28)
    .height(28)
    .justifyContent(FlexAlign.Center)
    .alignItems(VerticalAlign.Center)
    .onClick(onTap)
  }

  @Builder
  private symbolGlyph(symbolName: SystemSymbolName) {
    if (symbolName === 'chevron_down') {
      SymbolGlyph($r('sys.symbol.chevron_down'))
        .fontSize(18)
        .fontColor(['#2B2D35'])
    } else if (symbolName === 'mic_fill') {
      SymbolGlyph($r('sys.symbol.mic_fill'))
        .fontSize(16)
        .fontColor(['#2B2D35'])
    } else {
      SymbolGlyph($r('sys.symbol.chevron_up_circle'))
        .fontSize(20)
        .fontColor(['#2B2D35'])
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
        .fontColor('#2B2D35')
        .fontWeight(FontWeight.Bold)
    }
    .padding({ left: 16, right: 16, top: 9, bottom: 9 })
    .borderRadius(18)
    .backgroundColor('#FFFFFF')
    .shadow({ radius: 10, color: '#22000000', offsetX: 0, offsetY: 4 })
    .onClick(() => this.controller.triggerDestroy())
  }

  @Builder
  private windowContent() {
    Column({ space: 10 }) {
      if (this.controller.stopped) {
        Column() {
          this.stoppedContent();
        }
        .backgroundColor(Color.White)
        .borderRadius(26)
        .shadow({ radius: 18, color: '#26000000', offsetX: 0, offsetY: 8 })

        this.collapseButton();
      } else if (this.controller.mode === 'call_user_wait') {
        this.callUserWaitContent();
      } else if (this.controller.mode === 'call_user_input') {
        this.callUserInputContent();
      } else {
        Column() {
          this.activeContent();
        }
        .backgroundColor(Color.White)
        .borderRadius(26)
        .shadow({ radius: 18, color: '#26000000', offsetX: 0, offsetY: 8 })
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
