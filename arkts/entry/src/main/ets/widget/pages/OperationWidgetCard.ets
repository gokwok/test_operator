@Entry
@Component
struct OperationWidgetCard {
  build() {
    Column({ space: 10 }) {
      Text('操作智能体')
        .fontSize(14)
        .fontWeight(FontWeight.Medium)
        .fontColor('#1F2937');

      Column({ space: 8 }) {
        Row({ space: 8 }) {
          this.ShortcutButton('快捷 1');
          this.ShortcutButton('快捷 2');
        }
        .width('100%');

        Row({ space: 8 }) {
          this.ShortcutButton('快捷 3');
          this.ShortcutButton('快捷 4');
        }
        .width('100%');
      }
      .width('100%');
    }
    .padding(12)
    .width('100%')
    .height('100%')
    .backgroundColor('#FFFFFF');
  }

  @Builder
  private ShortcutButton(label: string) {
    Button(label)
      .type(ButtonType.Normal)
      .fontSize(14)
      .fontColor('#111827')
      .backgroundColor('#F3F4F6')
      .borderRadius(10)
      .layoutWeight(1)
      .height(44);
  }
}

