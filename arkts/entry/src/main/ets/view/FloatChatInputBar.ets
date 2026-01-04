@Component
export struct FloatChatInputBar {
  @Link text!: string;
  @Link sendSequence!: number;
  @Link micSequence!: number;
  @Prop placeholder: string = '点击进行输入';
  @Prop inputEnabled: boolean = true;
  @Prop isListening: boolean = false;
  @Prop listeningHint: string = '正在聆听...';

  @State private inputFocused: boolean = false;

  private readonly BAR_HEIGHT: number = 50;
  private readonly ICON_BTN_SIZE: number = 48;
  private readonly ACTIVE_COLOR: ResourceColor = '#00796B';
  private readonly INACTIVE_COLOR: ResourceColor = '#004D40';
  private readonly MUTED_COLOR: ResourceColor = '#6B8E94';

  build() {
    const trimmedText = this.text.trim();
    const micColor = this.isListening ? this.ACTIVE_COLOR : this.INACTIVE_COLOR;
    const micBgColor = this.isListening ? '#D7F2EE' : Color.Transparent;
    const textInputEnabled = this.inputEnabled && !this.isListening;
    const sendEnabled = this.inputEnabled && !this.isListening && trimmedText.length > 0;

    Row({ space: 4 }) {
      Button() {
        this.SystemSymbol('mic', 24, micColor, 'mic');
      }
      .width(this.ICON_BTN_SIZE)
      .height(this.ICON_BTN_SIZE)
      .backgroundColor(micBgColor)
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
        TextInput({ placeholder: '', text: this.text })
          .layoutWeight(1)
          .height('100%')
          .backgroundColor(Color.Transparent)
          .textAlign(TextAlign.Start)
          .fontColor(this.INACTIVE_COLOR)
          .caretColor(this.INACTIVE_COLOR)
          .fontSize(16)
          .padding({ left: 8 })
          .enabled(textInputEnabled)
          .onChange((value: string) => {
            this.text = value;
          })
          .onFocus(() => {
            this.inputFocused = true;
          })
          .onBlur(() => {
            this.inputFocused = false;
          })

        if (trimmedText.length === 0 && this.isListening) {
          Text(this.listeningHint)
            .fontSize(16)
            .fontColor(this.ACTIVE_COLOR)
            .width('100%')
            .height('100%')
            .textAlign(TextAlign.Center)
            .hitTestBehavior(HitTestMode.None)
        }

        if (trimmedText.length === 0 && !this.inputFocused && !this.isListening) {
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
          sendEnabled ? '#1D2129' : this.MUTED_COLOR,
          'paperplane_right_fill'
        );
      }
      .width(this.ICON_BTN_SIZE)
      .height(this.ICON_BTN_SIZE)
      .backgroundColor(Color.Transparent)
      .borderRadius(this.ICON_BTN_SIZE / 2)
      .enabled(sendEnabled)
      .opacity(sendEnabled ? 1 : 0.45)
      .onClick(() => {
        if (!this.inputEnabled) {
          return;
        }
        if (this.isListening || trimmedText.length === 0) {
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
      case 'paperplane_right_fill':
        return $r('sys.symbol.paperplane_right_fill');
      case 'mic':
        return $r('sys.symbol.mic');
      default:
        return $r('sys.symbol.mic');
    }
  }
}
