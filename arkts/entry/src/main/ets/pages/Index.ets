import { GlowOverlay, GlowPalette, GLOW_PALETTES } from '../ui_components/GlowOverlay';
import {
  InteractionEffectController,
  InteractionEffectOptions,
  InteractionEffectOverlay,
  InteractionPoint
} from '../ui_components/InteractionEffectOverlay';
import { FloatingOrb, FloatingOrbController, FloatingOrbMode } from '../ui_components/FloatingOrb';
import { StatusFloatWindow, StatusFloatWindowController } from '../ui_components/StatusFloatWindow';
import type { common, Want } from '@kit.AbilityKit';
import { AgentSpace } from '../features/agentspace/AgentSpace';
import type { AgentAction, ActionParameters, ActionResult, AgentSpaceRuntimeInfo } from '../features/agentspace/AgentSpace';
import type { AppTarget } from '../features/agentspace/VTSBackend';

@Entry
@Component
struct Index {
  @State paletteIndex: number = 0;
  @State showStatus: boolean = false;
  @State showGlow: boolean = false;
  @State vtsStatus: string = 'idle';
  @State vtsError: string = '';
  @State vtsScreenshotInfo: string = '';
  @State actionInput: string = 'hello';
  @State actionResult: string = '';
  @State targetBundleName: string = 'com.sina.weibo.stage';
  @State targetAbilityName: string = 'EntryAbility';
  private readonly initialStatus: string = 'hello world';
  private readonly statusResetDelayMs: number = 260;
  private statusResetTimerId: number = -1;
  private agentSpace: AgentSpace | null = null;

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

  aboutToAppear(): void {
    this.showGlow = false;
    this.showStatus = false;
    this.orbController.hide();
  }

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

          Row({ space: 8 }) {
            Button('Show Viewer')
              .fontSize(12)
              .fontColor(Color.White)
              .backgroundColor('#2A2C34')
              .borderRadius(14)
              .padding({ left: 12, right: 12, top: 6, bottom: 6 })
              .onClick(() => {
                void this.onShowViewer();
              })

            Button('Close Viewer')
              .fontSize(12)
              .fontColor(Color.White)
              .backgroundColor('#2A2C34')
              .borderRadius(14)
              .padding({ left: 12, right: 12, top: 6, bottom: 6 })
              .onClick(() => {
                void this.onCloseViewer();
              })
          }

          Column({ space: 8 }) {
            Text('Debug')
              .fontSize(12)
              .fontColor(Color.White)
              .fontWeight(FontWeight.Medium)

            Row({ space: 8 }) {
              Button('Create')
                .fontSize(12)
                .fontColor(Color.White)
                .backgroundColor('#2A2C34')
                .borderRadius(14)
                .padding({ left: 12, right: 12, top: 6, bottom: 6 })
                .onClick(() => {
                  void this.onVtsCreate();
                })

              Button('State')
                .fontSize(12)
                .fontColor(Color.White)
                .backgroundColor('#2A2C34')
                .borderRadius(14)
                .padding({ left: 12, right: 12, top: 6, bottom: 6 })
                .onClick(() => {
                  this.onVtsStatus();
                })

              Button('Launch')
                .fontSize(12)
                .fontColor(Color.White)
                .backgroundColor('#2A2C34')
                .borderRadius(14)
                .padding({ left: 12, right: 12, top: 6, bottom: 6 })
                .onClick(() => {
                  void this.onVtsLaunch();
                })

              Button('Destroy')
                .fontSize(12)
                .fontColor(Color.White)
                .backgroundColor('#2A2C34')
                .borderRadius(14)
                .padding({ left: 12, right: 12, top: 6, bottom: 6 })
                .onClick(() => {
                  void this.onVtsDestroy();
                })
            }

            Row({ space: 8 }) {
              Button('Click TL')
                .fontSize(12)
                .fontColor(Color.White)
                .backgroundColor('#2A2C34')
                .borderRadius(14)
                .padding({ left: 12, right: 12, top: 6, bottom: 6 })
                .onClick(() => {
                  void this.onVtsClickPreset(0.1, 0.2);
                })

              Button('Click C')
                .fontSize(12)
                .fontColor(Color.White)
                .backgroundColor('#2A2C34')
                .borderRadius(14)
                .padding({ left: 12, right: 12, top: 6, bottom: 6 })
                .onClick(() => {
                  void this.onVtsClickPreset(0.5, 0.5);
                })

              Button('Click BR')
                .fontSize(12)
                .fontColor(Color.White)
                .backgroundColor('#2A2C34')
                .borderRadius(14)
                .padding({ left: 12, right: 12, top: 6, bottom: 6 })
                .onClick(() => {
                  void this.onVtsClickPreset(0.9, 0.8);
                })
            }

            Row({ space: 8 }) {
              Button('Swipe Up')
                .fontSize(12)
                .fontColor(Color.White)
                .backgroundColor('#2A2C34')
                .borderRadius(14)
                .padding({ left: 12, right: 12, top: 6, bottom: 6 })
                .onClick(() => {
                  void this.onVtsSwipeUp();
                })

              Button('Shot')
                .fontSize(12)
                .fontColor(Color.White)
                .backgroundColor('#2A2C34')
                .borderRadius(14)
                .padding({ left: 12, right: 12, top: 6, bottom: 6 })
                .onClick(() => {
                  void this.onVtsScreenshot();
                })

              Button('Transfer')
                .fontSize(12)
                .fontColor(Color.White)
                .backgroundColor('#2A2C34')
                .borderRadius(14)
                .padding({ left: 12, right: 12, top: 6, bottom: 6 })
                .onClick(() => {
                  void this.onVtsTransfer();
                })
            }

            Row({ space: 8 }) {
              Button('Stop')
                .fontSize(12)
                .fontColor(Color.White)
                .backgroundColor('#2A2C34')
                .borderRadius(14)
                .padding({ left: 12, right: 12, top: 6, bottom: 6 })
                .onClick(() => {
                  void this.onVtsStop();
                })
            }

            Text('Action Debug')
              .fontSize(12)
              .fontColor(Color.White)
              .fontWeight(FontWeight.Medium)

            Row({ space: 8 }) {
              TextInput({ placeholder: 'type content', text: this.actionInput })
                .layoutWeight(1)
                .height(32)
                .fontSize(12)
                .fontColor(Color.White)
                .backgroundColor('#1A1B22')
                .borderRadius(10)
                .padding({ left: 8, right: 8 })
                .onChange((v: string) => {
                  this.actionInput = v;
                })
            }

            Row({ space: 8 }) {
              Button('Long Click')
                .fontSize(12)
                .fontColor(Color.White)
                .backgroundColor('#2A2C34')
                .borderRadius(14)
                .padding({ left: 12, right: 12, top: 6, bottom: 6 })
                .onClick(() => {
                  void this.onActionLongClick();
                })

              Button('Action Swipe')
                .fontSize(12)
                .fontColor(Color.White)
                .backgroundColor('#2A2C34')
                .borderRadius(14)
                .padding({ left: 12, right: 12, top: 6, bottom: 6 })
                .onClick(() => {
                  void this.onActionSwipe();
                })
            }

            Row({ space: 8 }) {
              Button('Type')
                .fontSize(12)
                .fontColor(Color.White)
                .backgroundColor('#2A2C34')
                .borderRadius(14)
                .padding({ left: 12, right: 12, top: 6, bottom: 6 })
                .onClick(() => {
                  void this.onActionType(false);
                })

              Button('Type @C')
                .fontSize(12)
                .fontColor(Color.White)
                .backgroundColor('#2A2C34')
                .borderRadius(14)
                .padding({ left: 12, right: 12, top: 6, bottom: 6 })
                .onClick(() => {
                  void this.onActionType(true);
                })

              Button('Back')
                .fontSize(12)
                .fontColor(Color.White)
                .backgroundColor('#2A2C34')
                .borderRadius(14)
                .padding({ left: 12, right: 12, top: 6, bottom: 6 })
                .onClick(() => {
                  void this.onActionPressBack();
                })

              Button('Enter')
                .fontSize(12)
                .fontColor(Color.White)
                .backgroundColor('#2A2C34')
                .borderRadius(14)
                .padding({ left: 12, right: 12, top: 6, bottom: 6 })
                .onClick(() => {
                  void this.onActionPressEnter();
                })
            }

            Row({ space: 8 }) {
              TextInput({ placeholder: 'bundleName', text: this.targetBundleName })
                .layoutWeight(1)
                .height(32)
                .fontSize(12)
                .fontColor(Color.White)
                .backgroundColor('#1A1B22')
                .borderRadius(10)
                .padding({ left: 8, right: 8 })
                .onChange((v: string) => {
                  this.targetBundleName = v;
                })

              TextInput({ placeholder: 'abilityName', text: this.targetAbilityName })
                .layoutWeight(1)
                .height(32)
                .fontSize(12)
                .fontColor(Color.White)
                .backgroundColor('#1A1B22')
                .borderRadius(10)
                .padding({ left: 8, right: 8 })
                .onChange((v: string) => {
                  this.targetAbilityName = v;
                })
            }

            if (this.vtsStatus.length > 0) {
              Text(this.vtsStatus)
                .fontSize(11)
                .fontColor('#B8C1CC')
            }

            if (this.vtsScreenshotInfo.length > 0) {
              Text(this.vtsScreenshotInfo)
                .fontSize(11)
                .fontColor('#B8C1CC')
            }

            if (this.actionResult.length > 0) {
              Text(this.actionResult)
                .fontSize(11)
                .fontColor('#B8C1CC')
            }

            if (this.vtsError.length > 0) {
              Text(this.vtsError)
                .fontSize(11)
                .fontColor('#F8B4B4')
            }
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

  private getAgentSpace(): AgentSpace {
    const ctx = getContext(this) as common.Context;
    if (!this.agentSpace) {
      this.agentSpace = AgentSpace.getInstance(ctx);
    } else {
      this.agentSpace.updateContext(ctx);
    }
    return this.agentSpace;
  }

  private async ensureRuntime(): Promise<AgentSpaceRuntimeInfo> {
    return await this.getAgentSpace().ensureVts();
  }

  private formatRuntime(rt: AgentSpaceRuntimeInfo): string {
    const displayId = rt.displayId === undefined ? 'n/a' : String(rt.displayId);
    return `screen=${rt.screenId} display=${displayId} size=${rt.widthPx}x${rt.heightPx} state=${rt.state} viewer=${rt.viewerActive} surface=${rt.surfaceAttached}`;
  }

  private async onVtsCreate(): Promise<void> {
    this.vtsError = '';
    try {
      const rt = await this.ensureRuntime();
      this.vtsStatus = this.formatRuntime(rt);
    } catch (e) {
      this.vtsError = `VTS create failed: ${String(e)}`;
    }
  }

  private onVtsStatus(): void {
    this.vtsError = '';
    try {
      const rt = this.getAgentSpace().getRuntimeInfo();
      this.vtsStatus = this.formatRuntime(rt);
    } catch (e) {
      this.vtsError = `VTS status failed: ${String(e)}`;
    }
  }

  private buildTarget(): AppTarget {
    const target: AppTarget = {
      bundleName: this.targetBundleName.trim(),
      abilityName: this.targetAbilityName.trim(),
    };
    return target;
  }

  private async onShowViewer(): Promise<void> {
    this.vtsError = '';
    try {
      await this.ensureRuntime();
      const ctx = getContext(this) as common.UIAbilityContext;
      const want: Want = {
        bundleName: 'com.huawei.aios.agent.operation',
        abilityName: 'AgentSpaceViewerAbility',
      };
      await ctx.startAbility(want);
    } catch (e) {
      this.vtsError = `Show viewer failed: ${String(e)}`;
    }
  }

  private async onCloseViewer(): Promise<void> {
    this.vtsError = '';
    try {
      const ctx = getContext(this) as common.UIAbilityContext;
      const params: Record<string, Object> = { closeViewer: true as unknown as Object };
      const want: Want = {
        bundleName: 'com.huawei.aios.agent.operation',
        abilityName: 'AgentSpaceViewerAbility',
        parameters: params,
      };
      await ctx.startAbility(want);
    } catch (e) {
      this.vtsError = `Close viewer failed: ${String(e)}`;
    }
  }

  private async onVtsLaunch(): Promise<void> {
    this.vtsError = '';
    try {
      await this.ensureRuntime();
      const target = this.buildTarget();
      if (target.bundleName.length === 0 || target.abilityName.length === 0) {
        this.vtsError = 'Target bundle/ability is empty';
        return;
      }
      await this.getAgentSpace().startApp(target);
      this.vtsStatus = `launch: ${target.bundleName}/${target.abilityName}`;
    } catch (e) {
      this.vtsError = `VTS launch failed: ${String(e)}`;
    }
  }

  private async onVtsClickCenter(): Promise<void> {
    this.vtsError = '';
    try {
      const rt = await this.ensureRuntime();
      const x = Math.floor(rt.widthPx / 2);
      const y = Math.floor(rt.heightPx / 2);
      await this.getAgentSpace().clickPx(x, y);
      this.vtsStatus = `click: (${x}, ${y})`;
    } catch (e) {
      this.vtsError = `VTS click failed: ${String(e)}`;
    }
  }

  private async onVtsClickPreset(rx: number, ry: number): Promise<void> {
    this.vtsError = '';
    try {
      const rt = await this.ensureRuntime();
      const x = this.ratioToPx(rt.widthPx, rx);
      const y = this.ratioToPx(rt.heightPx, ry);
      await this.getAgentSpace().clickPx(x, y);
      this.vtsStatus = `click: (${x}, ${y})`;
    } catch (e) {
      this.vtsError = `VTS click failed: ${String(e)}`;
    }
  }

  private async onVtsSwipeUp(): Promise<void> {
    this.vtsError = '';
    try {
      const rt = await this.ensureRuntime();
      const x = this.ratioToPx(rt.widthPx, 0.5);
      const startY = this.ratioToPx(rt.heightPx, 0.8);
      const endY = this.ratioToPx(rt.heightPx, 0.2);
      await this.getAgentSpace().swipePx(x, startY, x, endY);
      this.vtsStatus = `swipe: (${x}, ${startY}) -> (${x}, ${endY})`;
    } catch (e) {
      this.vtsError = `VTS swipe failed: ${String(e)}`;
    }
  }

  private async onVtsScreenshot(): Promise<void> {
    this.vtsError = '';
    try {
      await this.ensureRuntime();
      const startMs = Date.now();
      const raw = await this.getAgentSpace().screenshotBase64();
      const costMs = Date.now() - startMs;
      this.vtsScreenshotInfo = `screenshot len: ${raw.length} ms: ${costMs}`;
      if (raw.length === 0) {
        this.vtsError = 'Screenshot empty';
      }
    } catch (e) {
      this.vtsError = `VTS screenshot failed: ${String(e)}`;
    }
  }

  private async onVtsTransfer(): Promise<void> {
    this.vtsError = '';
    try {
      await this.ensureRuntime();
      const count = await this.getAgentSpace().transferToMain();
      this.vtsStatus = `transfer -> main (windows: ${count})`;
    } catch (e) {
      this.vtsError = `VTS transfer failed: ${String(e)}`;
    }
  }

  private async onVtsDestroy(): Promise<void> {
    this.vtsError = '';
    try {
      await this.getAgentSpace().destroy();
      this.vtsStatus = 'destroyed';
    } catch (e) {
      this.vtsError = `VTS destroy failed: ${String(e)}`;
    }
  }

  private async onVtsStop(): Promise<void> {
    this.vtsError = '';
    try {
      await this.ensureRuntime();
      const count = await this.getAgentSpace().transferToMain();
      await this.getAgentSpace().destroy();
      this.vtsStatus = `stopped (windows: ${count})`;
    } catch (e) {
      this.vtsError = `VTS stop failed: ${String(e)}`;
    }
  }

  private ratioToPx(limit: number, ratio: number): number {
    const clamped = Math.min(Math.max(ratio, 0), 1);
    const max = Math.max(1, limit);
    const px = Math.floor(clamped * (max - 1));
    return px < 0 ? 0 : px;
  }

  private async runAction(label: string, action: AgentAction): Promise<void> {
    this.vtsError = '';
    try {
      await this.ensureRuntime();
      const result: ActionResult = await this.getAgentSpace().executeAction(action);
      const msg = result.error ? ` error=${result.error}` : '';
      this.actionResult = `${label}: ${result.status}${msg}`;
    } catch (e) {
      this.vtsError = `Action ${label} failed: ${String(e)}`;
    }
  }

  private async onActionLongClick(): Promise<void> {
    const params: ActionParameters = {
      point: [500, 500],
      duration_ms: 1500,
    };
    const action: AgentAction = { name: 'long_click', parameters: params };
    await this.runAction('long_click', action);
  }

  private async onActionSwipe(): Promise<void> {
    const params: ActionParameters = {
      start_point: [500, 800],
      end_point: [500, 200],
      duration_ms: 300,
    };
    const action: AgentAction = { name: 'swipe', parameters: params };
    await this.runAction('swipe', action);
  }

  private async onActionType(usePoint: boolean): Promise<void> {
    const content = this.actionInput;
    if (content.length === 0) {
      this.vtsError = 'Type content is empty';
      return;
    }
    const params: ActionParameters = {
      content,
    };
    if (usePoint) {
      params.point = [500, 500];
    }
    const action: AgentAction = { name: 'type', parameters: params };
    await this.runAction('type', action);
  }

  private async onActionPressBack(): Promise<void> {
    const action: AgentAction = { name: 'press_back' };
    await this.runAction('press_back', action);
  }

  private async onActionPressEnter(): Promise<void> {
    const action: AgentAction = { name: 'press_enter' };
    await this.runAction('press_enter', action);
  }
}
