interface OperationWidgetCardActionParams {
  message: string;
}

interface OperationWidgetCardAction {
  action: string;
  params: OperationWidgetCardActionParams;
}

interface OperationWidgetCardMessage {
  type: string;
  skill_name: string;
}

declare function postCardAction(action: OperationWidgetCardAction): void;

@Entry
@Component
struct OperationWidgetCard {
  @State shortcut1: string = '快捷键1';
  @State shortcut2: string = '快捷键2';
  @State shortcut3: string = '快捷键3';
  @State shortcut4: string = '快捷键4';

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
      .height(44)
      .onClick(() => {
        this.handleShortcutClick(label);
      });
  }

  private handleShortcutClick(label: string): void {
    const trimmed = label.trim();
    if (trimmed.length === 0) {
      return;
    }
    if (trimmed.startsWith('快捷键')) {
      return;
    }
    const message: OperationWidgetCardMessage = { type: 'replay_skill', skill_name: trimmed };
    const params: OperationWidgetCardActionParams = { message: JSON.stringify(message) };
    const action: OperationWidgetCardAction = { action: 'message', params };
    postCardAction(action);
  }
}
