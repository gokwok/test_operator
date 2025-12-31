import { FloatChatInputBar } from '../ui_components/FloatChatInputBar';

@Entry
@Component
struct FloatChatPage {
  @State private inputText: string = '';
  @State private isVisible: boolean = false;
  @StorageProp('FloatWindow_IsClosing') @Watch('onClosingChange') private isClosing: boolean = false;

  private readonly SIDE_PADDING: number = 16;
  private readonly BOTTOM_PADDING: number = 24;
  private readonly ANIMATION_DURATION_MS: number = 350;
  private readonly ENTER_DELAY_MS: number = 50;
  private enterTimerId: number = -1;

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
          text: this.inputText,
          enabled: !this.isClosing,
          onTextChange: (value: string) => {
            this.inputText = value;
          },
          onMic: () => {},
          onSend: () => {},
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
}

