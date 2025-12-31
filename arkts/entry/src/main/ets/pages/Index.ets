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
import type { AppTarget } from '../features/agentspace/VTSBackend';
import { AgentServiceClient } from '../features/agent_service/ipc/AgentServiceClient';
import type { AppTargetDescriptor, TaskSnapshot, TaskStartRequest } from '../features/agent_service/types';
import { AppState } from '../features/state/app_state';
import { LiveViewController } from '../features/live_view/LiveViewController';
import { AgentSettings } from '../features/settings/AgentSettings';
import type { AgentSettingsPatch } from '../features/settings/AgentSettings';
import image from '@ohos.multimedia.image';
import abilityAccessCtrl from '@ohos.abilityAccessCtrl';
import photoAccessHelper from '@ohos.file.photoAccessHelper';
import util from '@ohos.util';

const VIEWER_BUNDLE_NAME: string = 'com.huawei.aios.agent.operation';
const VIEWER_ABILITY_NAME: string = 'AgentSpaceViewerAbility';
const FLOAT_WINDOW_ABILITY_NAME: string = 'FloatWindowAbility';
const VIEWER_CLOSE_ACTION: string = 'closeViewer';
const DEFAULT_CLOUD_ENDPOINT: string = 'http://114.55.173.20:8005';
const DEFAULT_WAIT_MS: number = 500;

@Entry
@Component
struct Index {
  @State activeTabIndex: number = 0;
  @State paletteIndex: number = 0;
  @State showStatus: boolean = false;
  @State showGlow: boolean = false;
  @State vtsStatus: string = 'idle';
  @State vtsError: string = '';
  @State vtsScreenshotInfo: string = '';
  @State screenshotPixelMap: image.PixelMap | null = null;
  @State actionInput: string = 'hello';
  @State actionResult: string = '';
  @State appListInfo: string = '';
  @State appResolveInfo: string = '';
  @State appIconInfo: string = '';
  @State appIconPixelMap: image.PixelMap | null = null;
  @State floatWindowInfo: string = '';
  @State liveViewInfo: string = '';
  @State liveViewTaskText: string = 'xxxxxxx';
  @State serviceStatus: string = '';
  @State serviceSnapshot: string = '';
  @State serviceError: string = '';
  @State serviceEvents: string = '';
  @State cloudEndpoint: string = DEFAULT_CLOUD_ENDPOINT;
  @State cloudTaskText: string = '';
  @State cloudSessionId: string = '';
  @State cloudWaitMs: string = String(DEFAULT_WAIT_MS);
  @State targetAppName: string = 'com.sina.weibo.stage';
  @State targetBundleName: string = 'com.sina.weibo.stage';
  @State targetAbilityName: string = 'EntryAbility';
  private readonly initialStatus: string = 'hello world';
  private readonly statusResetDelayMs: number = 260;
  private statusResetTimerId: number = -1;
  private serviceClient: AgentServiceClient | null = null;
  private screenshotBase64: string = '';
  private installedApps: Array<string> = [];
  private installedAppTargets: Array<AppTargetDescriptor> = [];
  private screenshotLongPressTimerId: number = -1;
  private screenshotPressX: number = 0;
  private screenshotPressY: number = 0;
  private readonly screenshotLongPressMs: number = 520;
  private readonly screenshotMoveThresholdPx: number = 12;

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
    void this.loadAgentSettings();
  }

  private getActivePalette(): GlowPalette {
    if (this.paletteIndex < 0 || this.paletteIndex >= this.palettes.length) {
      return this.palettes[0];
    }
    return this.palettes[this.paletteIndex];
  }

  @Builder
  private viewerTab(): void {
    Scroll() {
      Column({ space: 12 }) {
        Text('Viewer')
          .fontSize(12)
          .fontColor(Color.White)
          .fontWeight(FontWeight.Medium)

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

          Button('Viewer Control')
            .fontSize(12)
            .fontColor(Color.White)
            .backgroundColor('#2A2C34')
            .borderRadius(14)
            .padding({ left: 12, right: 12, top: 6, bottom: 6 })
            .onClick(() => {
              void this.onShowViewerControl();
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

        if (this.vtsError.length > 0) {
          Text(this.vtsError)
            .fontSize(11)
            .fontColor('#F8B4B4')
        }
      }
      .width('100%')
    }
    .scrollBar(BarState.On)
  }

  @Builder
  private floatWindowTab(): void {
    Scroll() {
      Column({ space: 12 }) {
        Text('System Float Window')
          .fontSize(12)
          .fontColor(Color.White)
          .fontWeight(FontWeight.Medium)

        Row({ space: 8 }) {
          Button('Show')
            .fontSize(12)
            .fontColor(Color.White)
            .backgroundColor('#2A2C34')
            .borderRadius(14)
            .padding({ left: 12, right: 12, top: 6, bottom: 6 })
            .onClick(() => {
              void this.onFloatWindowAction('show');
            })

          Button('Hide')
            .fontSize(12)
            .fontColor(Color.White)
            .backgroundColor('#2A2C34')
            .borderRadius(14)
            .padding({ left: 12, right: 12, top: 6, bottom: 6 })
            .onClick(() => {
              void this.onFloatWindowAction('hide');
            })

          Button('Toggle')
            .fontSize(12)
            .fontColor(Color.White)
            .backgroundColor('#2A2C34')
            .borderRadius(14)
            .padding({ left: 12, right: 12, top: 6, bottom: 6 })
            .onClick(() => {
              void this.onFloatWindowAction('toggle');
            })
        }

        if (this.floatWindowInfo.length > 0) {
          Text(this.floatWindowInfo)
            .fontSize(11)
            .fontColor('#E5E7EB')
        }
      }
      .width('100%')
    }
    .scrollBar(BarState.On)
  }

  @Builder
  private liveViewTab(): void {
    Scroll() {
      Column({ space: 12 }) {
        Text('LiveView')
          .fontSize(12)
          .fontColor(Color.White)
          .fontWeight(FontWeight.Medium)

        Text('primary.title: 任务执行中')
          .fontSize(11)
          .fontColor('#9CA3AF')

        TextInput({ placeholder: 'primary.content', text: this.liveViewTaskText })
          .width('100%')
          .height(40)
          .backgroundColor('#111217')
          .fontColor(Color.White)
          .placeholderColor('#6B7280')
          .borderRadius(10)
          .padding({ left: 12, right: 12 })
          .onChange((v: string) => {
            this.liveViewTaskText = v;
          })

        Text('capsule.title: 应用操作')
          .fontSize(11)
          .fontColor('#9CA3AF')

        Row({ space: 8 }) {
          Button('Start')
            .fontSize(12)
            .fontColor(Color.White)
            .backgroundColor('#2A2C34')
            .borderRadius(14)
            .padding({ left: 12, right: 12, top: 6, bottom: 6 })
            .onClick(() => {
              void this.onLiveViewStart();
            })

          Button('Update')
            .fontSize(12)
            .fontColor(Color.White)
            .backgroundColor('#2A2C34')
            .borderRadius(14)
            .padding({ left: 12, right: 12, top: 6, bottom: 6 })
            .onClick(() => {
              void this.onLiveViewUpdate();
            })

          Button('Stop')
            .fontSize(12)
            .fontColor(Color.White)
            .backgroundColor('#2A2C34')
            .borderRadius(14)
            .padding({ left: 12, right: 12, top: 6, bottom: 6 })
            .onClick(() => {
              void this.onLiveViewStop();
            })
        }

        if (this.liveViewInfo.length > 0) {
          Text(this.liveViewInfo)
            .fontSize(11)
            .fontColor('#E5E7EB')
        }
      }
      .width('100%')
    }
    .scrollBar(BarState.On)
  }

  @Builder
  private vtsTab(): void {
    Scroll() {
      Column({ space: 12 }) {
        Text('Service VTS')
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
              void this.onVtsStatus();
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
          Button('List App Names')
            .fontSize(12)
            .fontColor(Color.White)
            .backgroundColor('#2A2C34')
            .borderRadius(14)
            .padding({ left: 12, right: 12, top: 6, bottom: 6 })
            .onClick(() => {
              void this.onListApps();
            })
        }

        Row({ space: 8 }) {
          TextInput({ placeholder: 'appName (or bundleName)', text: this.targetAppName })
            .layoutWeight(1)
            .height(32)
            .fontSize(12)
            .fontColor(Color.White)
            .backgroundColor('#1A1B22')
            .borderRadius(10)
            .padding({ left: 8, right: 8 })
            .onChange((v: string) => {
              this.targetAppName = v;
            })

          Button('Resolve Launch')
            .fontSize(12)
            .fontColor(Color.White)
            .backgroundColor('#2A2C34')
            .borderRadius(14)
            .padding({ left: 12, right: 12, top: 6, bottom: 6 })
            .onClick(() => {
              void this.onResolveLaunchTarget();
            })

          Button('Show Icon')
            .fontSize(12)
            .fontColor(Color.White)
            .backgroundColor('#2A2C34')
            .borderRadius(14)
            .padding({ left: 12, right: 12, top: 6, bottom: 6 })
            .onClick(() => {
              void this.onShowAppIcon();
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
          Button('Delay Click C')
            .fontSize(12)
            .fontColor(Color.White)
            .backgroundColor('#2A2C34')
            .borderRadius(14)
            .padding({ left: 12, right: 12, top: 6, bottom: 6 })
            .onClick(() => {
              void this.onVtsClickPresetDelayed(0.5, 0.5, 1000);
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

        if (this.appListInfo.length > 0) {
          Text(this.appListInfo)
            .fontSize(11)
            .fontColor('#B8C1CC')
        }

        if (this.appResolveInfo.length > 0) {
          Text(this.appResolveInfo)
            .fontSize(11)
            .fontColor('#B8C1CC')
        }

        if (this.appIconInfo.length > 0) {
          Text(this.appIconInfo)
            .fontSize(11)
            .fontColor('#B8C1CC')
        }

        if (this.appIconPixelMap) {
          Image(this.appIconPixelMap)
            .width(72)
            .height(72)
            .borderRadius(12)
            .backgroundColor('#111217')
            .objectFit(ImageFit.Contain)
        }

        if (this.screenshotPixelMap) {
          Image(this.screenshotPixelMap)
            .width('100%')
            .height(220)
            .borderRadius(12)
            .backgroundColor('#111217')
            .objectFit(ImageFit.Contain)
            .onTouch((event: TouchEvent) => {
              this.handleScreenshotTouch(event);
            })
        }

        if (this.vtsError.length > 0) {
          Text(this.vtsError)
            .fontSize(11)
            .fontColor('#F8B4B4')
        }
      }
      .width('100%')
    }
    .scrollBar(BarState.On)
  }

  @Builder
  private serviceTab(): void {
    Scroll() {
      Column({ space: 12 }) {
        Text('Service')
          .fontSize(12)
          .fontColor(Color.White)
          .fontWeight(FontWeight.Medium)

        Row({ space: 8 }) {
          Button('Connect')
            .fontSize(12)
            .fontColor(Color.White)
            .backgroundColor('#2A2C34')
            .borderRadius(14)
            .padding({ left: 12, right: 12, top: 6, bottom: 6 })
            .onClick(() => {
              void this.onServiceConnect();
            })

          Button('Ping')
            .fontSize(12)
            .fontColor(Color.White)
            .backgroundColor('#2A2C34')
            .borderRadius(14)
            .padding({ left: 12, right: 12, top: 6, bottom: 6 })
            .onClick(() => {
              void this.onServicePing();
            })

          Button('Snapshot')
            .fontSize(12)
            .fontColor(Color.White)
            .backgroundColor('#2A2C34')
            .borderRadius(14)
            .padding({ left: 12, right: 12, top: 6, bottom: 6 })
            .onClick(() => {
              void this.onServiceSnapshot();
            })
        }

        Row({ space: 8 }) {
          TextInput({ placeholder: 'cloud endpoint', text: this.cloudEndpoint })
            .layoutWeight(1)
            .height(32)
            .fontSize(12)
            .fontColor(Color.White)
            .backgroundColor('#1A1B22')
            .borderRadius(10)
            .padding({ left: 8, right: 8 })
            .onChange((value: string) => {
              this.cloudEndpoint = value;
            })

          Button('Set Cloud')
            .fontSize(12)
            .fontColor(Color.White)
            .backgroundColor('#2A2C34')
            .borderRadius(14)
            .padding({ left: 12, right: 12, top: 6, bottom: 6 })
            .onClick(() => {
              void this.onServiceSetCloudEndpoint();
            })
        }

        Row({ space: 8 }) {
          TextInput({ placeholder: 'task text', text: this.cloudTaskText })
            .layoutWeight(1)
            .height(32)
            .fontSize(12)
            .fontColor(Color.White)
            .backgroundColor('#1A1B22')
            .borderRadius(10)
            .padding({ left: 8, right: 8 })
            .onChange((value: string) => {
              this.cloudTaskText = value;
            })
        }

        Row({ space: 8 }) {
          TextInput({ placeholder: 'session id (optional)', text: this.cloudSessionId })
            .layoutWeight(1)
            .height(32)
            .fontSize(12)
            .fontColor(Color.White)
            .backgroundColor('#1A1B22')
            .borderRadius(10)
            .padding({ left: 8, right: 8 })
            .onChange((value: string) => {
              this.cloudSessionId = value;
            })

          TextInput({ placeholder: 'wait_ms', text: this.cloudWaitMs })
            .width(90)
            .height(32)
            .fontSize(12)
            .fontColor(Color.White)
            .backgroundColor('#1A1B22')
            .borderRadius(10)
            .padding({ left: 8, right: 8 })
            .onChange((value: string) => {
              this.cloudWaitMs = value;
            })
        }

        Row({ space: 8 }) {
          Button('Start Cloud Task')
            .fontSize(12)
            .fontColor(Color.White)
            .backgroundColor('#2A2C34')
            .borderRadius(14)
            .padding({ left: 12, right: 12, top: 6, bottom: 6 })
            .onClick(() => {
              void this.onServiceStart();
            })

          Button('Stop Task')
            .fontSize(12)
            .fontColor(Color.White)
            .backgroundColor('#2A2C34')
            .borderRadius(14)
            .padding({ left: 12, right: 12, top: 6, bottom: 6 })
            .onClick(() => {
              void this.onServiceStop();
            })

          Button('Pull Events')
            .fontSize(12)
            .fontColor(Color.White)
            .backgroundColor('#2A2C34')
            .borderRadius(14)
            .padding({ left: 12, right: 12, top: 6, bottom: 6 })
            .onClick(() => {
              void this.onServicePullEvents();
            })
        }

        if (this.serviceStatus.length > 0) {
          Text(this.serviceStatus)
            .fontSize(11)
            .fontColor('#B8C1CC')
        }

        if (this.serviceSnapshot.length > 0) {
          Text(this.serviceSnapshot)
            .fontSize(11)
            .fontColor('#B8C1CC')
        }

        if (this.serviceEvents.length > 0) {
          Text(this.serviceEvents)
            .fontSize(11)
            .fontColor('#B8C1CC')
        }

        if (this.serviceError.length > 0) {
          Text(this.serviceError)
            .fontSize(11)
            .fontColor('#F8B4B4')
        }
      }
      .width('100%')
    }
    .scrollBar(BarState.On)
  }

  @Builder
  private actionTab(): void {
    Scroll() {
      Column({ space: 12 }) {
        Text('Actions')
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
          Button('Double Click')
            .fontSize(12)
            .fontColor(Color.White)
            .backgroundColor('#2A2C34')
            .borderRadius(14)
            .padding({ left: 12, right: 12, top: 6, bottom: 6 })
            .onClick(() => {
              void this.onActionDoubleClick();
            })

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
          Button('CallUser Debug')
            .fontSize(12)
            .fontColor(Color.White)
            .backgroundColor('#2A2C34')
            .borderRadius(14)
            .padding({ left: 12, right: 12, top: 6, bottom: 6 })
            .onClick(() => {
              void this.onActionDebugCallUser();
            })

          Button('TakeOver Debug')
            .fontSize(12)
            .fontColor(Color.White)
            .backgroundColor('#2A2C34')
            .borderRadius(14)
            .padding({ left: 12, right: 12, top: 6, bottom: 6 })
            .onClick(() => {
              void this.onActionDebugTakeOver();
            })

          Button('Interact Debug')
            .fontSize(12)
            .fontColor(Color.White)
            .backgroundColor('#2A2C34')
            .borderRadius(14)
            .padding({ left: 12, right: 12, top: 6, bottom: 6 })
            .onClick(() => {
              void this.onActionDebugInteract();
            })
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
      .width('100%')
    }
    .scrollBar(BarState.On)
  }

  @Builder
  private fxTab(): void {
    Scroll() {
      Column({ space: 12 }) {
        Text('FX')
          .fontSize(12)
          .fontColor(Color.White)
          .fontWeight(FontWeight.Medium)

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
      }
      .width('100%')
    }
    .scrollBar(BarState.On)
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
        Tabs({ index: this.activeTabIndex }) {
          TabContent() {
            this.viewerTab();
          }
          .tabBar('Viewer')

          TabContent() {
            this.floatWindowTab();
          }
          .tabBar('Float')

          TabContent() {
            this.liveViewTab();
          }
          .tabBar('LiveView')

          TabContent() {
            this.vtsTab();
          }
          .tabBar('Service VTS')

          TabContent() {
            this.serviceTab();
          }
          .tabBar('Service')

          TabContent() {
            this.actionTab();
          }
          .tabBar('Actions')

          TabContent() {
            this.fxTab();
          }
          .tabBar('FX')
        }
        .barPosition(BarPosition.Start)
        .scrollable(true)
        .onChange((index: number) => {
          this.activeTabIndex = index;
        })
        .width('100%')
        .height('100%')
        .padding({ left: 16, right: 16, top: 16, bottom: 24 })

        StatusFloatWindow({ controller: this.statusController, visible: this.showStatus })
          .width('100%')
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

  private getServiceClient(): AgentServiceClient {
    if (!this.serviceClient) {
      this.serviceClient = new AgentServiceClient();
    }
    return this.serviceClient;
  }

  private async onFloatWindowAction(action: string): Promise<void> {
    this.floatWindowInfo = '';
    const ctx = getContext(this) as common.UIAbilityContext;
    const want: Want = {
      bundleName: VIEWER_BUNDLE_NAME,
      abilityName: FLOAT_WINDOW_ABILITY_NAME,
      action,
    };
    try {
      await ctx.startServiceExtensionAbility(want);
      return;
    } catch (_e) {
      // fallback to startAbility for older environments
    }
    try {
      await ctx.startAbility(want);
    } catch (e) {
      this.floatWindowInfo = `start float window failed: ${String(e)}`;
    }
  }

  private async onLiveViewStart(): Promise<void> {
    this.liveViewInfo = 'Starting...';
    try {
      const ctx = getContext(this) as common.UIAbilityContext;
      const granted = await this.ensureBundleListPermission(ctx);
      if (!granted) {
        this.liveViewInfo = 'Bundle list permission not granted (icon may be missing).';
      }
      await LiveViewController.getInstance().startDefault(ctx, this.liveViewTaskText);
      this.liveViewInfo = 'LiveView started. Tap it to open Viewer.';
    } catch (e) {
      this.liveViewInfo = `LiveView start failed: ${String(e)}`;
    }
  }

  private async onLiveViewUpdate(): Promise<void> {
    this.liveViewInfo = 'Updating...';
    try {
      const ctx = getContext(this) as common.UIAbilityContext;
      const granted = await this.ensureBundleListPermission(ctx);
      if (!granted) {
        this.liveViewInfo = 'Bundle list permission not granted (icon may be missing).';
      }
      await LiveViewController.getInstance().updateDefault(ctx, this.liveViewTaskText);
      this.liveViewInfo = 'LiveView updated.';
    } catch (e) {
      this.liveViewInfo = `LiveView update failed: ${String(e)}`;
    }
  }

  private async onLiveViewStop(): Promise<void> {
    this.liveViewInfo = 'Stopping...';
    try {
      await this.ensureServiceConnected();
      const snapshotText = await this.getServiceClient().getSnapshot();
      const snapshot = this.parseTaskSnapshot(snapshotText);
      if (snapshot && (snapshot.status === 'running' || snapshot.status === 'waiting_user')) {
        await this.getServiceClient().stopTask('live_view_closed');
      }
      const ctx = getContext(this) as common.UIAbilityContext;
      await LiveViewController.getInstance().stopDefault(ctx);
      this.liveViewInfo = 'LiveView stopped.';
    } catch (e) {
      this.liveViewInfo = `LiveView stop failed: ${String(e)}`;
    }
  }

  private parseTaskSnapshot(payload: string): TaskSnapshot | null {
    if (payload.length === 0) {
      return null;
    }
    try {
      const parsed = JSON.parse(payload) as TaskSnapshot;
      if (!parsed || typeof parsed.status !== 'string') {
        return null;
      }
      return parsed;
    } catch {
      return null;
    }
  }

  private async onVtsCreate(): Promise<void> {
    this.vtsError = '';
    try {
      await this.ensureServiceConnected();
      const payload = await this.getServiceClient().ensureVts();
      this.vtsStatus = payload.length > 0 ? payload : 'vts ready';
    } catch (e) {
      this.vtsError = `VTS create failed: ${String(e)}`;
    }
  }

  private async onVtsStatus(): Promise<void> {
    this.vtsError = '';
    try {
      await this.ensureServiceConnected();
      const payload = await this.getServiceClient().getRuntime();
      this.vtsStatus = payload.length > 0 ? payload : 'runtime empty';
    } catch (e) {
      this.vtsError = `VTS status failed: ${String(e)}`;
    }
  }

  private async onListApps(): Promise<void> {
    this.vtsError = '';
    this.appListInfo = 'Loading app names...';
    this.appResolveInfo = '';
    this.appIconInfo = '';
    this.appIconPixelMap = null;
    try {
      const ctx = getContext(this) as common.UIAbilityContext;
      const granted = await this.ensureBundleListPermission(ctx);
      if (!granted) {
        this.appListInfo = '';
        if (this.vtsError.length === 0) {
          this.vtsError = 'Permission not granted. Open settings and retry.';
        }
        return;
      }
      const appNames = await AppState.listAppNames(ctx);
      if (appNames.length === 0) {
        this.appListInfo = 'No apps found';
        return;
      }
      this.installedApps = appNames;
      this.installedAppTargets = await AppState.listAppTargets(ctx);
      const preview = appNames.slice(0, 6).join(', ');
      this.appListInfo = `Apps: ${appNames.length}${preview.length > 0 ? ` | ${preview}` : ''}`;
    } catch (e) {
      this.appListInfo = '';
      this.vtsError = `List app names failed: ${String(e)}`;
    }
  }

  private async onResolveLaunchTarget(): Promise<void> {
    this.vtsError = '';
    this.appResolveInfo = 'Resolving launch target...';
    this.appIconInfo = '';
    this.appIconPixelMap = null;
    try {
      const ctx = getContext(this) as common.UIAbilityContext;
      const granted = await this.ensureBundleListPermission(ctx);
      if (!granted) {
        this.appResolveInfo = '';
        if (this.vtsError.length === 0) {
          this.vtsError = 'Permission not granted. Open settings and retry.';
        }
        return;
      }

      const target = await AppState.getLaunchTargetByAppName(this.targetAppName, ctx);
      this.targetBundleName = target.bundleName;
      this.targetAbilityName = target.abilityName;
      this.appResolveInfo = `Target: ${target.bundleName}/${target.abilityName}`;
    } catch (e) {
      this.appResolveInfo = '';
      this.vtsError = `Resolve launch target failed: ${String(e)}`;
    }
  }

  private async onShowAppIcon(): Promise<void> {
    this.vtsError = '';
    this.appIconInfo = 'Loading app icon...';
    this.appIconPixelMap = null;
    try {
      const ctx = getContext(this) as common.UIAbilityContext;
      const granted = await this.ensureBundleListPermission(ctx);
      if (!granted) {
        this.appIconInfo = '';
        if (this.vtsError.length === 0) {
          this.vtsError = 'Permission not granted. Open settings and retry.';
        }
        return;
      }

      const pixelMap = await AppState.getIconByAppName(this.targetAppName, ctx);
      this.appIconPixelMap = pixelMap;
      this.appIconInfo = `Icon: ${this.targetAppName}`;
    } catch (e) {
      this.appIconInfo = '';
      this.vtsError = `Get app icon failed: ${String(e)}`;
    }
  }

  private async onServiceConnect(): Promise<void> {
    this.serviceError = '';
    try {
      const ctx = getContext(this) as common.UIAbilityContext;
      await this.getServiceClient().connect(ctx);
      this.serviceStatus = 'connected';
    } catch (e) {
      this.serviceError = `Service connect failed: ${String(e)}`;
    }
  }

  private async onServicePing(): Promise<void> {
    this.serviceError = '';
    try {
      const pong = await this.getServiceClient().ping();
      this.serviceStatus = `ping: ${pong}`;
    } catch (e) {
      this.serviceError = `Service ping failed: ${String(e)}`;
    }
  }

  private async onServiceSnapshot(): Promise<void> {
    this.serviceError = '';
    try {
      const snapshot = await this.getServiceClient().getSnapshot();
      this.serviceSnapshot = snapshot;
      this.serviceStatus = 'snapshot ok';
    } catch (e) {
      this.serviceError = `Service snapshot failed: ${String(e)}`;
    }
  }

  private async onServiceSetCloudEndpoint(): Promise<void> {
    this.serviceError = '';
    try {
      const endpoint = this.cloudEndpoint.trim();
      if (endpoint.length === 0) {
        this.serviceError = 'Cloud endpoint is empty';
        return;
      }
      await this.ensureServiceConnected();
      await this.getServiceClient().setCloudEndpoint(endpoint);
      this.serviceStatus = `cloud: ${endpoint}`;
      await this.saveAgentSettings({ cloudEndpoint: endpoint, waitMs: this.readWaitMs() });
    } catch (e) {
      this.serviceError = `Cloud endpoint failed: ${String(e)}`;
    }
  }

  private async onServiceStart(): Promise<void> {
    this.serviceError = '';
    try {
      const taskText = this.cloudTaskText.trim();
      if (taskText.length === 0) {
        this.serviceError = 'Task text is empty';
        return;
      }
      await this.ensureServiceConnected();
      const endpoint = this.cloudEndpoint.trim();
      if (endpoint.length > 0) {
        await this.getServiceClient().setCloudEndpoint(endpoint);
      }
      const waitMsRaw = parseInt(this.cloudWaitMs, 10);
      const waitMs = Number.isFinite(waitMsRaw) ? Math.max(0, waitMsRaw) : DEFAULT_WAIT_MS;
      await this.saveAgentSettings({ cloudEndpoint: endpoint, waitMs });
      const request: TaskStartRequest = {
        sessionId: this.cloudSessionId.trim(),
        taskText,
        installedApps: this.installedApps.slice(),
        appTargets: this.installedAppTargets.slice(),
        waitMs,
      };
      await this.getServiceClient().startTask(request);
      this.serviceEvents = '';
      this.serviceStatus = 'cloud task started';
    } catch (e) {
      this.serviceError = `Service start failed: ${String(e)}`;
    }
  }

  private async onServiceStop(): Promise<void> {
    this.serviceError = '';
    try {
      await this.getServiceClient().stopTask('user_stop');
      this.serviceStatus = 'stopped';
    } catch (e) {
      this.serviceError = `Service stop failed: ${String(e)}`;
    }
  }

  private async loadAgentSettings(): Promise<void> {
    try {
      const ctx = getContext(this) as common.UIAbilityContext;
      const settings = await AgentSettings.load(ctx);
      this.cloudEndpoint = settings.cloudEndpoint;
      this.cloudWaitMs = String(settings.waitMs);
    } catch (e) {
      console.warn(`[Index] load settings failed: ${String(e)}`);
    }
  }

  private readWaitMs(): number {
    const waitMsRaw = parseInt(this.cloudWaitMs, 10);
    if (!Number.isFinite(waitMsRaw)) {
      return DEFAULT_WAIT_MS;
    }
    return Math.max(0, waitMsRaw);
  }

  private async saveAgentSettings(patch: AgentSettingsPatch): Promise<void> {
    try {
      const ctx = getContext(this) as common.UIAbilityContext;
      await AgentSettings.update(ctx, patch);
    } catch (e) {
      console.warn(`[Index] save settings failed: ${String(e)}`);
    }
  }

  private async onServicePullEvents(): Promise<void> {
    this.serviceError = '';
    try {
      const payload = await this.getServiceClient().pullEvents(0);
      if (payload.length === 0) {
        this.serviceStatus = 'no events';
        return;
      }
      this.serviceEvents = payload;
      this.serviceStatus = 'events ok';
    } catch (e) {
      this.serviceError = `Service events failed: ${String(e)}`;
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
      await this.ensureServiceConnected();
      await this.getServiceClient().setViewerMode('display');
      await this.getServiceClient().ensureVts();
      await this.startViewerAbility();
    } catch (e) {
      this.vtsError = `Show viewer failed: ${String(e)}`;
    }
  }

  private async onCloseViewer(): Promise<void> {
    this.vtsError = '';
    try {
      await this.ensureServiceConnected();
      await this.getServiceClient().setViewerActive(false);
      await this.startViewerAbility(VIEWER_CLOSE_ACTION);
    } catch (e) {
      this.vtsError = `Close viewer failed: ${String(e)}`;
    }
  }

  private async onShowViewerControl(): Promise<void> {
    this.vtsError = '';
    try {
      await this.ensureServiceConnected();
      await this.getServiceClient().setViewerMode('control');
      await this.getServiceClient().ensureVts();
      await this.startViewerAbility();
    } catch (e) {
      this.vtsError = `Show viewer control failed: ${String(e)}`;
    }
  }

  private async onVtsLaunch(): Promise<void> {
    this.vtsError = '';
    try {
      const target = this.buildTarget();
      if (target.bundleName.length === 0 || target.abilityName.length === 0) {
        this.vtsError = 'Target bundle/ability is empty';
        return;
      }
      await this.ensureServiceConnected();
      await this.getServiceClient().launchApp(target.bundleName, target.abilityName);
      this.vtsStatus = `launch: ${target.bundleName}/${target.abilityName}`;
    } catch (e) {
      this.vtsError = `VTS launch failed: ${String(e)}`;
    }
  }

  private async onVtsClickCenter(): Promise<void> {
    await this.onVtsClickPreset(0.5, 0.5);
  }

  private async onVtsClickPreset(rx: number, ry: number): Promise<void> {
    this.vtsError = '';
    try {
      await this.ensureServiceConnected();
      const xNorm = this.ratioToNorm(rx);
      const yNorm = this.ratioToNorm(ry);
      await this.getServiceClient().clickNorm(xNorm, yNorm);
      this.vtsStatus = `click: (${xNorm}, ${yNorm})`;
    } catch (e) {
      this.vtsError = `VTS click failed: ${String(e)}`;
    }
  }

  private async onVtsClickPresetDelayed(rx: number, ry: number, delayMs: number): Promise<void> {
    this.vtsError = '';
    try {
      const waitMs = Math.max(0, Math.floor(delayMs));
      this.vtsStatus = `click in ${waitMs}ms`;
      await new Promise<void>((resolve: () => void) => {
        setTimeout(() => resolve(), waitMs);
      });
      await this.ensureServiceConnected();
      const xNorm = this.ratioToNorm(rx);
      const yNorm = this.ratioToNorm(ry);
      await this.getServiceClient().clickNorm(xNorm, yNorm);
      this.vtsStatus = `click: (${xNorm}, ${yNorm})`;
    } catch (e) {
      this.vtsError = `VTS click failed: ${String(e)}`;
    }
  }

  private async onVtsSwipeUp(): Promise<void> {
    this.vtsError = '';
    try {
      await this.ensureServiceConnected();
      const xNorm = this.ratioToNorm(0.5);
      const startY = this.ratioToNorm(0.8);
      const endY = this.ratioToNorm(0.2);
      const result = await this.getServiceClient().actionSwipe(xNorm, startY, xNorm, endY, 300);
      this.vtsStatus = `swipe: ${result}`;
    } catch (e) {
      this.vtsError = `VTS swipe failed: ${String(e)}`;
    }
  }

  private async onVtsScreenshot(): Promise<void> {
    this.vtsError = '';
    try {
      await this.ensureServiceConnected();
      const startMs = Date.now();
      const raw = await this.getServiceClient().screenshot();
      const costMs = Date.now() - startMs;
      this.vtsScreenshotInfo = `screenshot len: ${raw.length} ms: ${costMs}`;
      this.screenshotBase64 = raw;
      if (raw.length === 0) {
        this.vtsError = 'Screenshot empty';
        this.screenshotPixelMap = null;
        this.screenshotBase64 = '';
        return;
      }
      await this.updateScreenshotPreview(raw);
    } catch (e) {
      this.vtsError = `VTS screenshot failed: ${String(e)}`;
    }
  }

  private async updateScreenshotPreview(base64: string): Promise<void> {
    if (base64.length === 0) {
      this.screenshotPixelMap = null;
      this.screenshotBase64 = '';
      return;
    }
    let source: image.ImageSource | null = null;
    try {
      const helper = new util.Base64Helper();
      const decoded = helper.decodeSync(base64);
      const buffer = decoded.buffer.slice(decoded.byteOffset, decoded.byteOffset + decoded.byteLength);
      source = image.createImageSource(buffer);
      const pixelMap = await source.createPixelMap();
      this.screenshotPixelMap = pixelMap;
    } catch (e) {
      this.screenshotPixelMap = null;
      this.vtsError = `Screenshot decode failed: ${String(e)}`;
    } finally {
      if (source) {
        try {
          await source.release();
        } catch {
          // Ignore release failures.
        }
      }
    }
  }

  private handleScreenshotTouch(event: TouchEvent): void {
    if (!this.screenshotPixelMap || this.screenshotBase64.length === 0) {
      return;
    }
    const touch = this.getPrimaryTouch(event);
    if (event.type === TouchType.Down) {
      if (touch) {
        this.screenshotPressX = touch.windowX;
        this.screenshotPressY = touch.windowY;
      }
      this.resetScreenshotLongPress();
      this.screenshotLongPressTimerId = setTimeout(() => {
        this.screenshotLongPressTimerId = -1;
        void this.saveScreenshotToAlbum();
      }, this.screenshotLongPressMs);
      return;
    }
    if (event.type === TouchType.Move && touch) {
      const dx = touch.windowX - this.screenshotPressX;
      const dy = touch.windowY - this.screenshotPressY;
      const distance = Math.sqrt(dx * dx + dy * dy);
      if (distance > this.screenshotMoveThresholdPx) {
        this.resetScreenshotLongPress();
      }
      return;
    }
    if (event.type === TouchType.Up || event.type === TouchType.Cancel) {
      this.resetScreenshotLongPress();
    }
  }

  private resetScreenshotLongPress(): void {
    if (this.screenshotLongPressTimerId !== -1) {
      clearTimeout(this.screenshotLongPressTimerId);
      this.screenshotLongPressTimerId = -1;
    }
  }

  private async saveScreenshotToAlbum(): Promise<void> {
    if (this.screenshotBase64.length === 0) {
      this.vtsError = 'No screenshot to save';
      return;
    }
    this.vtsError = '';
    try {
      const ctx = getContext(this) as common.UIAbilityContext;
      const granted = await this.ensureWriteImagePermission(ctx);
      if (!granted) {
        this.vtsError = 'Permission denied for album save';
        return;
      }
      const helper = photoAccessHelper.getPhotoAccessHelper(ctx);
      const request = photoAccessHelper.MediaAssetChangeRequest.createAssetRequest(
        ctx,
        photoAccessHelper.PhotoType.IMAGE,
        'jpg'
      );
      const base64 = new util.Base64Helper();
      const decoded = base64.decodeSync(this.screenshotBase64);
      const buffer = decoded.buffer.slice(decoded.byteOffset, decoded.byteOffset + decoded.byteLength);
      request.addResource(photoAccessHelper.ResourceType.IMAGE_RESOURCE, buffer);
      await helper.applyChanges(request);
      this.vtsStatus = 'Saved to album';
    } catch (e) {
      this.vtsError = `Save screenshot failed: ${String(e)}`;
    }
  }

  private async ensureBundleListPermission(ctx: common.UIAbilityContext): Promise<boolean> {
    const atManager = abilityAccessCtrl.createAtManager();
    const tokenId = ctx.applicationInfo.accessTokenId;
    const permission = 'ohos.permission.GET_INSTALLED_BUNDLE_LIST';
    try {
      const status = await atManager.checkAccessToken(tokenId, permission);
      if (status === abilityAccessCtrl.GrantStatus.PERMISSION_GRANTED) {
        return true;
      }
      const result = await atManager.requestPermissionsFromUser(ctx, [permission]);
      if (result.authResults && result.authResults.length > 0) {
        if (result.authResults[0] === abilityAccessCtrl.GrantStatus.PERMISSION_GRANTED) {
          return true;
        }
      }
      await atManager.requestPermissionOnApplicationSetting(tokenId);
      return false;
    } catch (e) {
      this.vtsError = `Permission request failed: ${String(e)}`;
      return false;
    }
  }

  private async ensureWriteImagePermission(ctx: common.UIAbilityContext): Promise<boolean> {
    const atManager = abilityAccessCtrl.createAtManager();
    const tokenId = ctx.applicationInfo.accessTokenId;
    try {
      const status = await atManager.checkAccessToken(tokenId, 'ohos.permission.WRITE_IMAGEVIDEO');
      if (status === abilityAccessCtrl.GrantStatus.PERMISSION_GRANTED) {
        return true;
      }
      const result = await atManager.requestPermissionsFromUser(ctx, ['ohos.permission.WRITE_IMAGEVIDEO']);
      if (!result.authResults || result.authResults.length === 0) {
        return false;
      }
      return result.authResults[0] === abilityAccessCtrl.GrantStatus.PERMISSION_GRANTED;
    } catch (e) {
      this.vtsError = `Permission request failed: ${String(e)}`;
      return false;
    }
  }

  private getPrimaryTouch(event: TouchEvent): TouchObject | null {
    const changed = event.changedTouches;
    if (changed && changed.length > 0) {
      return changed[0];
    }
    const touches = event.touches;
    if (touches && touches.length > 0) {
      return touches[0];
    }
    return null;
  }

  private async onVtsTransfer(): Promise<void> {
    this.vtsError = '';
    try {
      await this.ensureServiceConnected();
      const count = await this.getServiceClient().transferToMain();
      this.vtsStatus = `transfer -> main (windows: ${count})`;
    } catch (e) {
      this.vtsError = `VTS transfer failed: ${String(e)}`;
    }
  }

  private async onVtsDestroy(): Promise<void> {
    this.vtsError = '';
    try {
      await this.ensureServiceConnected();
      await this.getServiceClient().destroyVts();
      this.vtsStatus = 'destroyed';
    } catch (e) {
      this.vtsError = `VTS destroy failed: ${String(e)}`;
    }
  }

  private async onVtsStop(): Promise<void> {
    this.vtsError = '';
    try {
      await this.ensureServiceConnected();
      const count = await this.getServiceClient().stopVts();
      this.vtsStatus = `stopped (windows: ${count})`;
    } catch (e) {
      this.vtsError = `VTS stop failed: ${String(e)}`;
    }
  }

  private ratioToNorm(ratio: number): number {
    const clamped = Math.min(Math.max(ratio, 0), 1);
    return Math.round(clamped * 1000);
  }

  private async ensureServiceConnected(): Promise<void> {
    const ctx = getContext(this) as common.UIAbilityContext;
    await this.getServiceClient().connect(ctx);
  }

  private async startViewerAbility(action?: string): Promise<void> {
    const ctx = getContext(this) as common.UIAbilityContext;
    const want: Want = {
      bundleName: VIEWER_BUNDLE_NAME,
      abilityName: VIEWER_ABILITY_NAME,
    };
    if (action && action.length > 0) {
      want.action = action;
    }
    await ctx.startAbility(want);
  }

  private async onActionLongClick(): Promise<void> {
    this.vtsError = '';
    try {
      await this.ensureServiceConnected();
      const result = await this.getServiceClient().actionLongClick(500, 500, 1500);
      this.actionResult = `long_click: ${result}`;
    } catch (e) {
      this.vtsError = `Action long_click failed: ${String(e)}`;
    }
  }

  private async onActionDoubleClick(): Promise<void> {
    this.vtsError = '';
    try {
      await this.ensureServiceConnected();
      const result = await this.getServiceClient().actionDoubleClick(500, 500);
      this.actionResult = `double_click: ${result}`;
    } catch (e) {
      this.vtsError = `Action double_click failed: ${String(e)}`;
    }
  }

  private async onActionSwipe(): Promise<void> {
    this.vtsError = '';
    try {
      await this.ensureServiceConnected();
      const result = await this.getServiceClient().actionSwipe(500, 800, 500, 200, 300);
      this.actionResult = `swipe: ${result}`;
    } catch (e) {
      this.vtsError = `Action swipe failed: ${String(e)}`;
    }
  }

  private async onActionType(usePoint: boolean): Promise<void> {
    const content = this.actionInput;
    if (content.length === 0) {
      this.vtsError = 'Type content is empty';
      return;
    }
    this.vtsError = '';
    try {
      await this.ensureServiceConnected();
      const result = await this.getServiceClient().actionType(content, usePoint, 500, 500);
      this.actionResult = `type: ${result}`;
    } catch (e) {
      this.vtsError = `Action type failed: ${String(e)}`;
    }
  }

  private async onActionPressBack(): Promise<void> {
    this.vtsError = '';
    try {
      await this.ensureServiceConnected();
      const result = await this.getServiceClient().actionPressBack();
      this.actionResult = `press_back: ${result}`;
    } catch (e) {
      this.vtsError = `Action press_back failed: ${String(e)}`;
    }
  }

  private async onActionPressEnter(): Promise<void> {
    this.vtsError = '';
    try {
      await this.ensureServiceConnected();
      const result = await this.getServiceClient().actionPressEnter();
      this.actionResult = `press_enter: ${result}`;
    } catch (e) {
      this.vtsError = `Action press_enter failed: ${String(e)}`;
    }
  }

  private async onActionDebugCallUser(): Promise<void> {
    const content = this.actionInput.trim();
    if (content.length === 0) {
      this.vtsError = 'CallUser content is empty';
      return;
    }
    this.vtsError = '';
    try {
      await this.ensureServiceConnected();
      await this.getServiceClient().debugCallUser(content);
      this.actionResult = 'call_user dispatched';
    } catch (e) {
      this.vtsError = `Debug call_user failed: ${String(e)}`;
    }
  }

  private async onActionDebugTakeOver(): Promise<void> {
    const content = this.actionInput.trim();
    if (content.length === 0) {
      this.vtsError = 'TakeOver content is empty';
      return;
    }
    this.vtsError = '';
    try {
      await this.ensureServiceConnected();
      await this.getServiceClient().debugTakeOver(content);
      this.actionResult = 'take_over dispatched';
    } catch (e) {
      this.vtsError = `Debug take_over failed: ${String(e)}`;
    }
  }

  private async onActionDebugInteract(): Promise<void> {
    const content = this.actionInput.trim();
    if (content.length === 0) {
      this.vtsError = 'Interact content is empty';
      return;
    }
    this.vtsError = '';
    try {
      await this.ensureServiceConnected();
      const options: Array<string> = ['选项 A', '选项 B', '选项 C'];
      await this.getServiceClient().debugInteract(content, options);
      this.actionResult = 'interact dispatched';
    } catch (e) {
      this.vtsError = `Debug interact failed: ${String(e)}`;
    }
  }
}
