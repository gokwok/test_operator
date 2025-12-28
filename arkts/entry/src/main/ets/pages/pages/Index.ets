import { GlowOverlay, GlowPalette, GLOW_PALETTES } from '../ui_components/GlowOverlay';
import {
  InteractionEffectController,
  InteractionEffectOptions,
  InteractionEffectOverlay,
  InteractionPoint
} from '../ui_components/InteractionEffectOverlay';
import { FloatingOrb, FloatingOrbController, FloatingOrbMode } from '../ui_components/FloatingOrb';
import { StatusFloatWindow, StatusFloatWindowController } from '../ui_components/StatusFloatWindow';

@Entry
@Component
struct Index {
  @State message: string = 'Hello World';
  @State paletteIndex: number = 0;
  @State showStatus: boolean = true;
  @State showGlow: boolean = true;
  private readonly initialStatus: string = 'hello world';
  private readonly statusResetDelayMs: number = 260;
  private statusResetTimerId: number = -1;

  private readonly palettes: GlowPalette[] = GLOW_PALETTES;
  @State private statusController: StatusFloatWindowController = StatusFloatWindowController.create(
    this.initialStatus,
    () => {},
    () => {},
    () => {},
    () => {
      this.showGlow = false;
      this.showStatus = false;
      if (this.statusResetTimerId !== -1) {
        clearTimeout(this.statusResetTimerId);
      }
      this.statusResetTimerId = setTimeout(() => {
        this.statusController.setStopped(false);
        this.statusController.setStatus(this.initialStatus);
        this.statusResetTimerId = -1;
      }, this.statusResetDelayMs);
    }
  );
  @State private orbController: FloatingOrbController = FloatingOrbController.create(
    '任务即将恢复 (4)',
    'expanded',
    () => {}
  );
  @State private interactionController: InteractionEffectController = InteractionEffectController.create();
  private readonly tapPoint: InteractionPoint = { x: 300, y: 500 };
  private readonly swipeStart: InteractionPoint = { x: 200, y: 700 };
  private readonly swipeEnd: InteractionPoint = { x: 800, y: 300 };
  private readonly swipeOptions: InteractionEffectOptions = { durationMs: 800 };

  private getActivePalette(): GlowPalette {
    if (this.paletteIndex < 0 || this.paletteIndex >= this.palettes.length) {
      return this.palettes[0];
    }
    return this.palettes[this.paletteIndex];
  }

  build() {
    Stack() {
      Column()
        .width('100%')
        .height('100%')
        .backgroundColor('#090A0F')

      GlowOverlay({ paletteName: this.getActivePalette().name, visible: this.showGlow })
        .width('100%')
        .height('100%')

      InteractionEffectOverlay({ controller: this.interactionController })
        .width('100%')
        .height('100%')

      RelativeContainer() {
        Text(this.message)
          .id('HelloWorld')
          .fontSize($r('app.float.page_text_font_size'))
          .fontWeight(FontWeight.Bold)
          .fontColor(Color.White)
          .alignRules({
            center: { anchor: '__container__', align: VerticalAlign.Center },
            middle: { anchor: '__container__', align: HorizontalAlign.Center }
          })
          .onClick(() => {
            this.message = this.message === 'Hello World' ? 'Welcome' : 'Hello World';
          })

        Column({ space: 12 }) {
          Row({ space: 8 }) {
            ForEach(this.palettes, (item: GlowPalette, index: number) => {
              Button(item.name)
                .fontSize(12)
                .fontColor(this.paletteIndex === index ? Color.Black : Color.White)
                .backgroundColor(this.paletteIndex === index ? '#F2F2F2' : '#1A1B22')
                .borderRadius(16)
                .padding({ left: 12, right: 12, top: 6, bottom: 6 })
                .onClick(() => {
                  this.paletteIndex = index;
                })
            }, (item: GlowPalette) => item.name)
          }

          Row({ space: 8 }) {
            Button('Status: short')
              .fontSize(12)
              .fontColor(Color.White)
              .backgroundColor('#2A2C34')
              .borderRadius(14)
              .padding({ left: 12, right: 12, top: 6, bottom: 6 })
              .onClick(() => {
                this.statusController.setStatus('hello world');
              })

            Button('Status: alert')
              .fontSize(12)
              .fontColor(Color.White)
              .backgroundColor('#2A2C34')
              .borderRadius(14)
              .padding({ left: 12, right: 12, top: 6, bottom: 6 })
              .onClick(() => {
                this.statusController.setStatus('- **Thinkflow**：智能体“程序/包”的抽象（交付物 + 入口 + 资源声明 + 运行配置）。Thinkflow 不定义推理循环，不绑定框架。\n' +
                  '- **Thinkflow Runtime（TFR）**：承载 Thinkflow 的执行抽象，类似线程/actor。负责会话与运行生命周期、**会话级隔离**、调度与事件输出，以及把 AIOS 资源以 localhost 端点注入。\n' +
                  '- **AIOS**：平台底座，提供并治理智能资源（LPU/Memory/Tools 等）。');
              })

            Button('Status: long')
              .fontSize(12)
              .fontColor(Color.White)
              .backgroundColor('#2A2C34')
              .borderRadius(14)
              .padding({ left: 12, right: 12, top: 6, bottom: 6 })
              .onClick(() => {
                this.statusController.setStatus(
                  'This is a longer status message that should wrap onto a second line to verify auto height.'
                );
              })
          }

          Row({ space: 8 }) {
            Button(this.showStatus ? 'Hide Status' : 'Show Status')
              .fontSize(12)
              .fontColor(Color.White)
              .backgroundColor('#2A2C34')
              .borderRadius(14)
              .padding({ left: 12, right: 12, top: 6, bottom: 6 })
              .onClick(() => {
                this.showStatus = !this.showStatus;
              })

            Button(this.showGlow ? 'Hide Glow' : 'Show Glow')
              .fontSize(12)
              .fontColor(Color.White)
              .backgroundColor('#2A2C34')
              .borderRadius(14)
              .padding({ left: 12, right: 12, top: 6, bottom: 6 })
              .onClick(() => {
                this.showGlow = !this.showGlow;
              })
          }

          Row({ space: 8 }) {
            Button(this.orbController.visible ? 'Hide Orb' : 'Show Orb')
              .fontSize(12)
              .fontColor(Color.White)
              .backgroundColor('#2A2C34')
              .borderRadius(14)
              .padding({ left: 12, right: 12, top: 6, bottom: 6 })
              .onClick(() => {
                this.orbController.toggle();
              })

            Button('Orb Mode')
              .fontSize(12)
              .fontColor(Color.White)
              .backgroundColor('#2A2C34')
              .borderRadius(14)
              .padding({ left: 12, right: 12, top: 6, bottom: 6 })
              .onClick(() => {
                const nextMode: FloatingOrbMode =
                  this.orbController.mode === 'expanded' ? 'compact' : 'expanded';
                this.orbController.setMode(nextMode);
              })

            Button('Orb Text')
              .fontSize(12)
              .fontColor(Color.White)
              .backgroundColor('#2A2C34')
              .borderRadius(14)
              .padding({ left: 12, right: 12, top: 6, bottom: 6 })
              .onClick(() => {
                const nextText =
                  this.orbController.text === '任务即将恢复 (4)' ? '继续播放' : '任务即将恢复 (4)';
                this.orbController.setText(nextText);
              })
          }

          Row({ space: 8 }) {
            Button('Click')
              .fontSize(12)
              .fontColor(Color.White)
              .backgroundColor('#2A2C34')
              .borderRadius(14)
              .padding({ left: 12, right: 12, top: 6, bottom: 6 })
              .onClick(() => {
                this.interactionController.tap(this.tapPoint);
              })

            Button('LongPress')
              .fontSize(12)
              .fontColor(Color.White)
              .backgroundColor('#2A2C34')
              .borderRadius(14)
              .padding({ left: 12, right: 12, top: 6, bottom: 6 })
              .onClick(() => {
                this.interactionController.longPress(this.tapPoint);
              })

            Button('Swipe')
              .fontSize(12)
              .fontColor(Color.White)
              .backgroundColor('#2A2C34')
              .borderRadius(14)
              .padding({ left: 12, right: 12, top: 6, bottom: 6 })
              .onClick(() => {
                this.interactionController.swipe(this.swipeStart, this.swipeEnd, this.swipeOptions);
              })
          }

          StatusFloatWindow({ controller: this.statusController, visible: this.showStatus })
            .width('100%')
        }
        .width('100%')
        .padding({ left: 16, right: 16, bottom: 24 })
        .alignRules({
          bottom: { anchor: '__container__', align: VerticalAlign.Bottom },
          middle: { anchor: '__container__', align: HorizontalAlign.Center }
        })
      }
      .width('100%')
      .height('100%')

      FloatingOrb({ controller: this.orbController, visible: this.orbController.visible })
    }
    .height('100%')
    .width('100%')
  }
}
