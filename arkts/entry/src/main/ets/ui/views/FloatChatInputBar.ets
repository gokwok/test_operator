type InputMode = 'voice' | 'keyboard';

@Component
export struct FloatChatInputBar {
  @Link text!: string;
  @Link sendSequence!: number;
  @Link micSequence!: number;
  @Link pttStartSequence!: number;
  @Link pttStopSequence!: number;
  @Link pttPressed!: boolean;
  @Prop @Watch('onFocusSequence') focusSequence: number = 0;
  @Prop placeholder: string = '点击进行输入';
  @Prop inputEnabled: boolean = true;
  @Prop @Watch('onInputModeChange') inputMode: InputMode = 'voice';
  @Prop @Watch('onListeningChange') isListening: boolean = false;
  @Prop asrBubbleText: string = '';
  @Prop isTransferring: boolean = false;
  @Prop transferText: string = '';
  @Prop transferProgress: number = 0;

  @State private inputFocused: boolean = false;
  @State private pendingFocus: boolean = false;
  private readonly INPUT_FOCUS_ID: string = 'float_chat_input';
  private focusRetryTimerId: number = -1;

  private readonly BAR_HEIGHT: number = 50;
  private readonly ICON_BTN_SIZE: number = 48;
  private readonly ACTIVE_COLOR: ResourceColor = '#00796B';
  private readonly INACTIVE_COLOR: ResourceColor = '#004D40';
  private readonly MUTED_COLOR: ResourceColor = '#6B8E94';

  build() {
    Column({ space: 10 }) {
      if (this.shouldShowAsrBubble()) {
        Text(this.getAsrBubbleDisplayText())
          .fontSize(15)
          .fontColor(this.getAsrBubbleFontColor())
          .padding({ left: 12, right: 12, top: 10, bottom: 10 })
          .backgroundColor('#FFFFFF')
          .borderRadius(14)
          .opacity(this.getAsrBubbleOpacity())
          .translate({ y: this.getAsrBubbleTranslateY() })
          .shadow({ radius: 16, color: 'rgba(0, 77, 64, 0.12)', offsetX: 0, offsetY: 6 })
          .width('100%')
      }

      Row({ space: 4 }) {
        Button() {
          if (this.isListening) {
            this.SystemSymbol('mic', 24, this.resolveLeftIconColor(), 'mic_listening');
          } else if (this.inputMode === 'voice') {
            this.SystemSymbol('input_mode', 24, this.resolveLeftIconColor(), 'input_mode');
          } else {
            this.SystemSymbol('mic', 24, this.resolveLeftIconColor(), 'mic_idle');
          }
        }
        .width(this.ICON_BTN_SIZE)
        .height(this.ICON_BTN_SIZE)
        .backgroundColor(this.resolveLeftIconBgColor())
        .borderRadius(this.ICON_BTN_SIZE / 2)
        .enabled(this.inputEnabled)
        .opacity(this.inputEnabled ? 1 : 0.45)
        .onClick(() => {
          if (!this.inputEnabled) {
            return;
          }
          this.micSequence += 1;
        })

        Stack() {
          if (this.inputMode === 'voice') {
            Button() {
              Text((this.isListening || this.pttPressed) ? '正在识别' : '按住说话')
                .fontSize(16)
                .fontColor(this.inputEnabled ? '#1D2129' : this.MUTED_COLOR)
            }
            .width('100%')
            .height('100%')
            .backgroundColor(Color.Transparent)
            .enabled(this.inputEnabled)
            .opacity(this.inputEnabled ? 1 : 0.45)
            .onTouch((event: TouchEvent) => {
              this.handlePttTouch(event);
            })
          } else {
            TextInput({ placeholder: '', text: this.text })
              .id(this.INPUT_FOCUS_ID)
              .layoutWeight(1)
              .height('100%')
              .backgroundColor(Color.Transparent)
              .textAlign(TextAlign.Start)
              .fontColor(this.INACTIVE_COLOR)
              .caretColor(this.INACTIVE_COLOR)
              .fontSize(16)
              .padding({ left: 8 })
              .enableKeyboardOnFocus(true)
              .enabled(this.isTextInputEnabled())
              .onChange((value: string) => {
                this.text = value;
              })
              .onFocus(() => {
                this.inputFocused = true;
              })
              .onBlur(() => {
                this.inputFocused = false;
              })

            if (this.shouldShowPlaceholder()) {
              Text(this.placeholder)
                .fontSize(16)
                .fontColor(this.MUTED_COLOR)
                .width('100%')
                .height('100%')
                .textAlign(TextAlign.Center)
                .hitTestBehavior(HitTestMode.None)
            }
          }

          if (this.shouldShowTransferringOverlay()) {
            Text(this.transferText)
              .fontSize(16)
              .fontColor('#1D2129')
              .width('100%')
              .height('100%')
              .textAlign(TextAlign.Start)
              .padding({ left: 8 })
              .opacity(1 - this.transferProgress)
              .translate({ y: -16 + 16 * this.transferProgress })
              .hitTestBehavior(HitTestMode.None)
          }
        }
        .layoutWeight(1)

        Button() {
          this.SystemSymbol(
            'paperplane_right_fill',
            24,
            this.isSendEnabled() ? '#1D2129' : this.MUTED_COLOR,
            'paperplane_right_fill'
          );
        }
        .width(this.ICON_BTN_SIZE)
        .height(this.ICON_BTN_SIZE)
        .backgroundColor(Color.Transparent)
        .borderRadius(this.ICON_BTN_SIZE / 2)
        .enabled(this.isSendEnabled())
        .opacity(this.isSendEnabled() ? 1 : 0.45)
        .onClick(() => {
          if (!this.inputEnabled) {
            return;
          }
          if (this.isListening || this.getTrimmedText().length === 0) {
            return;
          }
          this.sendSequence += 1;
        })
      }
      .width('100%')
      .height(this.BAR_HEIGHT)
      .padding({ left: 8, right: 8 })
      .linearGradient({
        angle: 180,
        colors: [['#EEF9FB', 0.0], ['#D8EFF5', 1.0]]
      })
      .borderRadius(36)
      .shadow({ radius: 20, color: 'rgba(0, 77, 64, 0.15)', offsetX: 0, offsetY: 8 })
      .hitTestBehavior(HitTestMode.Default)
    }
    .width('100%')
  }

  private getTrimmedText(): string {
    return this.text.trim();
  }

  private resolveLeftIconColor(): ResourceColor {
    if (!this.inputEnabled) {
      return this.MUTED_COLOR;
    }
    return this.INACTIVE_COLOR;
  }

  private resolveLeftIconBgColor(): ResourceColor {
    return Color.Transparent;
  }

  private handlePttTouch(event: TouchEvent): void {
    if (!this.inputEnabled || this.inputMode !== 'voice') {
      return;
    }
    switch (event.type) {
      case TouchType.Down:
        this.onPttDown();
        break;
      case TouchType.Up:
        this.onPttUpOrCancel();
        break;
      case TouchType.Cancel:
        this.onPttUpOrCancel();
        break;
      default:
        break;
    }
  }

  private onPttDown(): void {
    if (this.pttPressed) {
      return;
    }
    this.pttPressed = true;
    if (this.isListening) {
      return;
    }
    this.pttStartSequence += 1;
  }

  private onPttUpOrCancel(): void {
    if (!this.pttPressed) {
      return;
    }
    this.pttPressed = false;
    if (!this.isListening && this.asrBubbleText.trim().length === 0) {
      return;
    }
    this.pttStopSequence += 1;
  }

  private onFocusSequence(): void {
    this.pendingFocus = true;
    this.tryFocusInput();
  }

  private onListeningChange(): void {
    this.tryFocusInput();
  }

  private onInputModeChange(): void {
    if (this.inputMode !== 'voice' && this.pttPressed) {
      this.pttPressed = false;
    }
    this.tryFocusInput();
  }

  private tryFocusInput(): void {
    if (!this.pendingFocus) {
      if (this.focusRetryTimerId !== -1) {
        clearTimeout(this.focusRetryTimerId);
        this.focusRetryTimerId = -1;
      }
      return;
    }
    if (!this.isTextInputEnabled()) {
      return;
    }
    if (focusControl.requestFocus(this.INPUT_FOCUS_ID)) {
      this.pendingFocus = false;
      if (this.focusRetryTimerId !== -1) {
        clearTimeout(this.focusRetryTimerId);
        this.focusRetryTimerId = -1;
      }
      return;
    }
    if (this.focusRetryTimerId !== -1) {
      return;
    }
    this.focusRetryTimerId = setTimeout(() => {
      this.focusRetryTimerId = -1;
      this.tryFocusInput();
    }, 16) as number;
  }

  private isTextInputEnabled(): boolean {
    return this.inputEnabled && !this.isListening && this.inputMode === 'keyboard';
  }

  private isSendEnabled(): boolean {
    return this.inputEnabled && !this.isListening && this.getTrimmedText().length > 0;
  }

  private shouldShowPlaceholder(): boolean {
    return this.getTrimmedText().length === 0 && !this.inputFocused && !this.isListening && this.inputMode === 'keyboard';
  }

  private shouldShowAsrBubble(): boolean {
    if (this.isTransferring && this.transferText.trim().length > 0) {
      return true;
    }
    if (this.inputMode !== 'voice') {
      return false;
    }
    if (this.isListening) {
      return true;
    }
    return this.asrBubbleText.trim().length > 0;
  }

  private getAsrBubbleDisplayText(): string {
    if (this.isTransferring) {
      return this.transferText;
    }
    if (this.asrBubbleText.trim().length === 0) {
      return '...';
    }
    return this.asrBubbleText;
  }

  private getAsrBubbleFontColor(): ResourceColor {
    if (this.isTransferring) {
      return '#1D2129';
    }
    if (this.asrBubbleText.trim().length === 0) {
      return '#86909C';
    }
    return '#1D2129';
  }

  private getAsrBubbleOpacity(): number {
    if (!this.isTransferring) {
      return 1;
    }
    return 1 - this.transferProgress;
  }

  private getAsrBubbleTranslateY(): number {
    if (!this.isTransferring) {
      return 0;
    }
    return 18 * this.transferProgress;
  }

  private shouldShowTransferringOverlay(): boolean {
    return this.isTransferring && this.inputMode === 'keyboard' && this.transferText.trim().length > 0;
  }

  @Builder
  private SystemSymbol(name: string, size: number, color: ResourceColor, key: string) {
    Text() {
      SymbolSpan(this.getSymbolResource(name))
        .fontSize(size)
        .fontColor([color])
    }
    .key(key)
    .width(size)
    .height(size)
    .textAlign(TextAlign.Center)
    .hitTestBehavior(HitTestMode.None)
  }

  private getSymbolResource(name: string): Resource {
    switch (name) {
      case 'input_mode':
        return $r('sys.symbol.input_mode');
      case 'paperplane_right_fill':
        return $r('sys.symbol.paperplane_right_fill');
      case 'mic':
        return $r('sys.symbol.mic');
      default:
        return $r('sys.symbol.mic');
    }
  }
}
