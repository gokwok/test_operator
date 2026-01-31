@Entry
@Component
struct OperationWidgetCard {
  @LocalStorageProp('shortcut1') shortcut1: string = '快捷键1';
  @LocalStorageProp('shortcut2') shortcut2: string = '快捷键2';
  @LocalStorageProp('shortcut3') shortcut3: string = '快捷键3';
  @LocalStorageProp('shortcut4') shortcut4: string = '快捷键4';

  build() {
    Column({ space: 8 }) {
      Row({ space: 8 }) {
        this.ShortcutButton(this.shortcut1);
        this.ShortcutButton(this.shortcut2);
      }
      .width('100%');

      Row({ space: 8 }) {
        this.ShortcutButton(this.shortcut3);
        this.ShortcutButton(this.shortcut4);
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
    Button() {
      Text(label)
        .maxLines(1)
        .textOverflow({ overflow: TextOverflow.Ellipsis })
    }
      .type(ButtonType.Normal)
      .fontSize(14)
      .fontColor('#111827')
      .backgroundColor('#F3F4F6')
      .borderRadius(10)
      .layoutWeight(1)
      .height(44);
  }
}
