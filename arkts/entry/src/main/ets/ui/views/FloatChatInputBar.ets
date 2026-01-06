type InputMode = 'voice' | 'keyboard';

@Component
export struct FloatChatInputBar {
  @Link text!: string;
  @Link sendSequence!: number;
  @Link micSequence!: number;
  @Prop @Watch('onFocusSequence') focusSequence: number = 0;
  @Prop placeholder: string = '点击进行输入';
  @Prop inputEnabled: boolean = true;
  @Prop @Watch('onInputModeChange') inputMode: InputMode = 'voice';
  @Prop @Watch('onListeningChange') isListening: boolean = false;
  @Prop listeningHint: string = '正在聆听...';

  @State private inputFocused: boolean = false;
  @State private pendingFocus: boolean = false;
  private readonly INPUT_FOCUS_ID: string = 'float_chat_input';

  private readonly BAR_HEIGHT: number = 50;
  private readonly ICON_BTN_SIZE: number = 48;
  private readonly ACTIVE_COLOR: ResourceColor = '#00796B';
  private readonly INACTIVE_COLOR: ResourceColor = '#004D40';
  private readonly MUTED_COLOR: ResourceColor = '#6B8E94';

  build() {
    const leftSymbolName = this.resolveLeftSymbolName();
    Row({ space: 4 }) {
      Button() {
        this.SystemSymbol(leftSymbolName, 24, this.resolveLeftIconColor(), leftSymbolName);
      }
      .width(this.ICON_BTN_SIZE)
      .height(this.ICON_BTN_SIZE)
      .backgroundColor(this.resolveLeftIconBgColor())
      .borderRadius(this.ICON_BTN_SIZE / 2)
      .enabled(this.inputEnabled)
      .opacity(this.inputEnabled ? 1 : 0.45)
      .onClick(() => {
        this.handleLeftButtonClick();
      })

      Stack() {
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

        if (this.shouldShowListeningHint()) {
          Text(this.listeningHint)
            .fontSize(16)
            .fontColor(this.ACTIVE_COLOR)
            .width('100%')
            .height('100%')
            .textAlign(TextAlign.Center)
            .hitTestBehavior(HitTestMode.None)
        }

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

  private getTrimmedText(): string {
    return this.text.trim();
  }

  private resolveLeftSymbolName(): string {
    if (this.isListening) {
      return 'mic';
    }
    return this.inputMode === 'voice' ? 'input_mode' : 'mic';
  }

  private resolveLeftIconColor(): ResourceColor {
    if (!this.inputEnabled) {
      return this.MUTED_COLOR;
    }
    return this.isListening ? this.ACTIVE_COLOR : this.INACTIVE_COLOR;
  }

  private resolveLeftIconBgColor(): ResourceColor {
    if (!this.inputEnabled) {
      return Color.Transparent;
    }
    return this.isListening ? '#D7F2EE' : Color.Transparent;
  }

  private handleLeftButtonClick(): void {
    if (!this.inputEnabled) {
      return;
    }
    this.micSequence += 1;
  }

  private onFocusSequence(): void {
    this.pendingFocus = true;
    this.tryFocusInput();
  }

  private onListeningChange(): void {
    this.tryFocusInput();
  }

  private onInputModeChange(): void {
    this.tryFocusInput();
  }

  private tryFocusInput(): void {
    if (!this.pendingFocus) {
      return;
    }
    if (!this.isTextInputEnabled()) {
      return;
    }
    if (focusControl.requestFocus(this.INPUT_FOCUS_ID)) {
      this.pendingFocus = false;
    }
  }

  private isTextInputEnabled(): boolean {
    return this.inputEnabled && !this.isListening && this.inputMode === 'keyboard';
  }

  private isSendEnabled(): boolean {
    return this.inputEnabled && !this.isListening && this.getTrimmedText().length > 0;
  }

  private shouldShowListeningHint(): boolean {
    return this.getTrimmedText().length === 0 && this.isListening;
  }

  private shouldShowPlaceholder(): boolean {
    return this.getTrimmedText().length === 0 && !this.inputFocused && !this.isListening && this.inputMode === 'keyboard';
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
