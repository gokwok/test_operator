@Component
export struct FloatChatInputBar {
  @Prop text: string = '';
  @Prop placeholder: string = '点击进行输入';
  @Prop inputEnabled: boolean = true;
  @Prop onTextChange: (text: string) => void = (_text: string) => {};
  @Prop onMic: () => void = () => {};
  @Prop onSend: () => void = () => {};

  @State private inputFocused: boolean = false;

  private readonly BAR_HEIGHT: number = 50;
  private readonly ICON_BTN_SIZE: number = 48;

  build() {
    Row({ space: 4 }) {
      Button() {
        this.SystemSymbol('mic', 24, '#004D40', 'mic');
      }
      .width(this.ICON_BTN_SIZE)
      .height(this.ICON_BTN_SIZE)
      .backgroundColor(Color.Transparent)
      .borderRadius(this.ICON_BTN_SIZE / 2)
      .enabled(this.inputEnabled)
      .opacity(this.inputEnabled ? 1 : 0.45)
      .onClick(() => {
        this.onMic();
      })

      Stack() {
        TextInput({ placeholder: '', text: this.text })
          .layoutWeight(1)
          .height('100%')
          .backgroundColor(Color.Transparent)
          .textAlign(TextAlign.Start)
          .fontColor('#004D40')
          .caretColor('#004D40')
          .fontSize(16)
          .padding({ left: 8 })
          .enabled(this.inputEnabled)
          .onChange((value: string) => {
            this.onTextChange(value);
          })
          .onFocus(() => {
            this.inputFocused = true;
          })
          .onBlur(() => {
            this.inputFocused = false;
          })

        if (this.text.trim().length === 0 && !this.inputFocused) {
          Text(this.placeholder)
            .fontSize(16)
            .fontColor('#6B8E94')
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
          this.text.trim().length > 0 ? '#1D2129' : '#6B8E94',
          'paperplane_right_fill'
        );
      }
      .width(this.ICON_BTN_SIZE)
      .height(this.ICON_BTN_SIZE)
      .backgroundColor(Color.Transparent)
      .borderRadius(this.ICON_BTN_SIZE / 2)
      .enabled(this.inputEnabled && this.text.trim().length > 0)
      .opacity(this.inputEnabled && this.text.trim().length > 0 ? 1 : 0.45)
      .onClick(() => {
        this.onSend();
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
