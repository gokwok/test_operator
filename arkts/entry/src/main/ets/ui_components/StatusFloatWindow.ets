import { FadeVisibility } from './FadeVisibility';

type ActionType = 'supplement' | 'takeover' | 'stop';

@Observed
export class StatusFloatWindowController {
  statusText: string = '';
  stopped: boolean = false;

  private supplementHandler: () => void = () => {};
  private takeoverHandler: () => void = () => {};
  private stopHandler: () => void = () => {};
  private destroyHandler: () => void = () => {};

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
}

@Component
export struct StatusFloatWindow {
  @ObjectLink controller: StatusFloatWindowController;
  @Prop visible: boolean = true;

  private readonly stopTitle: string = '用户已停止任务执行';
  private readonly stopDescription: string = '用户主动停止了当前任务的执行。';
  private readonly collapseLabel: string = '收起';

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
      Column() {
        if (this.controller.stopped) {
          this.stoppedContent();
        } else {
          this.activeContent();
        }
      }
      .backgroundColor(Color.White)
      .borderRadius(26)
      .shadow({ radius: 18, color: '#26000000', offsetX: 0, offsetY: 8 })

      if (this.controller.stopped) {
        this.collapseButton();
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
