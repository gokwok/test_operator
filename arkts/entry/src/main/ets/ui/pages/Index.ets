import { abilityAccessCtrl } from '@kit.AbilityKit';
import type { common, Permissions, Want } from '@kit.AbilityKit';
import type { BusinessError } from '@kit.BasicServicesKit';
import image from '@ohos.multimedia.image';

import { TaskHistoryStore } from '../../features/persistence/history/TaskHistoryStore';
import type { HistoryStepMeta, HistoryTaskMeta } from '../../features/persistence/history/TaskHistoryStore';
import { AgentSettings } from '../../features/persistence/settings/AgentSettings';
import type { AgentSettingsData } from '../../features/persistence/settings/AgentSettings';
import { WidgetFormStore } from '../../features/persistence/widget/WidgetFormStore';
import type { WidgetFormRecord } from '../../features/persistence/widget/WidgetFormStore';
import { OperationWidgetUpdater } from '../../features/persistence/widget/OperationWidgetUpdater';
import { AgentControllerClient } from '../../features/agent_controller/ipc/AgentControllerClient';
import type { TaskEvent, TaskEventBatch } from '../../features/agent_controller/types';
import { WallpaperState } from '../../features/agent_controller/state_collector/WallpaperState';
import { ForegroundAppState } from '../../features/agent_controller/state_collector/ForegroundAppState';
import { LocationState } from '../../features/agent_controller/state_collector/LocationState';

const DEFAULT_TASK_LIST_LIMIT: number = 100;
const DEFAULT_STEP_LIST_LIMIT: number = 200;
const THEME_BG_START: string = '#FFFFFF';
const THEME_PANEL: string = '#FFFFFF';
const THEME_CARD: string = '#FFFFFF';
const THEME_BORDER: string = '#E6DDF1';
const THEME_ACCENT: string = '#3D6AE8';
const THEME_ACCENT_SOFT: string = '#E6EEFF';
const THEME_TEXT_PRIMARY: string = '#2B1E35';
const THEME_TEXT_MUTED: string = '#7C6F8D';
const THEME_TEXT_SUBTLE: string = '#B1A6C4';
const THEME_NAV_ACTIVE_BG: string = '#E9EFFF';
const THEME_ERROR: string = '#D94848';
const FONT_DISPLAY: string = 'HarmonyOS Sans SC';
const SAFE_TOP_PADDING: number = 24;
const NAV_HEIGHT: number = 96;
const NAV_BLUR_RADIUS: number = 18;
const TASK_BAR_HEIGHT: number = 48;
const TASK_BAR_ICON_SIZE: number = 18;
const TASK_BAR_ACTION_WIDTH: number = 76;
const TASK_CARD_SUCCESS_BG: string = '#EAF8EE';
const TASK_CARD_SUCCESS_BORDER: string = '#6FBF88';
const TASK_CARD_FAIL_BG: string = '#FCEAEA';
const TASK_CARD_FAIL_BORDER: string = '#E18181';
const TASK_CARD_ICON_SIZE: number = 18;
const TASK_CARD_MIN_HEIGHT: number = 140;
const TASK_CARD_SIDE_MARGIN: number = 12;
const TASK_CARD_PADDING_X: number = 12;
const TASK_CARD_PADDING_TOP: number = 12;
const TASK_CARD_PADDING_BOTTOM: number = 10;
const TASK_SWIPE_ACTION_WIDTH: number = 72;
const TASK_FAVORITE_COLOR: string = '#F2C94C';
const NAV_SAFE_SPACER: number = 12;
const DETAIL_CARD_BG: string = '#FFFFFF';
const DETAIL_CARD_BORDER: string = '#ECE5F5';
const DETAIL_SECTION_BG: string = '#F6F7FB';
const DETAIL_SECTION_BORDER: string = '#E8E1F2';
const DETAIL_TITLE_SIZE: number = 13;
const DETAIL_TEXT_SIZE: number = 11;
const DETAIL_HINT_TEXT: string = '#9B93A8';
const DETAIL_TAG_BG: string = '#EEF2FF';
const DETAIL_TAG_TEXT: string = '#3D6AE8';
const DETAIL_STATUS_OK_BG: string = '#E3F4EA';
const DETAIL_STATUS_OK_TEXT: string = '#2F7A4E';
const DETAIL_STATUS_FAIL_BG: string = '#FBE6E6';
const DETAIL_STATUS_FAIL_TEXT: string = '#B04A4A';
const DETAIL_CARD_RADIUS: number = 18;
const DETAIL_SECTION_RADIUS: number = 12;
const STEP_IMAGE_HEIGHT: number = 520;
const STEP_SWIPER_HEIGHT: number = 720;
type JsonLike = object | string | number | boolean | null;

interface AgentActionSnapshot {
  name: string;
  parameters?: AgentActionParametersSnapshot;
}

interface AgentActionParametersSnapshot {
  point?: Array<number>;
  start_point?: Array<number>;
  end_point?: Array<number>;
  duration_ms?: number;
  seconds?: number;
  content?: string;
  app?: string;
  options?: Array<string>;
}

interface StepParamLine {
  label: string;
  value: string;
}

interface McpStatusSnapshot {
  state?: string;
  url?: string;
  error?: string;
}

interface A2AStatusMessagePartSnapshot {
  text?: string;
}

interface A2AStatusMessageSnapshot {
  parts?: Array<A2AStatusMessagePartSnapshot>;
}

interface A2AStatusSnapshot {
  state?: string;
  message?: A2AStatusMessageSnapshot;
}

interface A2ATaskSnapshot {
  id?: string;
  contextId?: string;
  status?: A2AStatusSnapshot;
}

interface A2AResponseSnapshot {
  task?: A2ATaskSnapshot;
  id?: string;
  status?: A2AStatusSnapshot;
}

interface ServiceExtensionStarter {
  startServiceExtensionAbility?: (want: Want) => Promise<void>;
}

type NavSymbolName = 'house_fill' | 'gearshape' | 'bolt_filled_on_circle';
type TaskBarSymbolName = 'arrow_clockwise' | 'clean' | 'chevron_left' | 'trash';

@Entry
@Component
struct Index {
  @State activeTabIndex: number = 0;

  @State cloudEndpoint: string = '';
  @State a2aBaseUrl: string = '';
  @State mcpWsUrl: string = '';
  @State deviceId: string = '';
  @State contextId: string = '';
  @State waitMsText: string = '';
  @State liveViewEnabled: boolean = true;
  @State historyEnabled: boolean = true;
  @State historyMaxTasksText: string = '50';
  @State settingsInfo: string = '';
  @State wallpaperInfo: string = '';
  @State wallpaperPixelMap: image.PixelMap | null = null;
  @State foregroundAppInfo: string = '';
  @State locationInfo: string = '';
  @State agentSpaceInfo: string = '';
  @State agentSpaceAppName: string = '';
  @State a11yTreeListInfo: string = '';
  @State debugEventInfo: string = '';
  @State debugEvents: Array<TaskEvent> = [];
  @State debugToolName: string = 'screenshot';
  @State debugToolArgs: string = '{}';
  @State debugToolResult: string = '';
  @State mcpStatusInfo: string = '';
  @State mcpLogInfo: string = '';
  @State mcpLogLines: Array<string> = [];
  @State a2aPrompt: string = '';
  @State a2aTaskId: string = '';
  @State a2aReply: string = '';
  @State a2aResultInfo: string = '';
  @State a2aResult: string = '';
  @State a2aStreamPrompt: string = '';
  @State a2aStreamContextId: string = '';
  @State a2aStreamInfo: string = '';
  @State a2aStreamEvents: Array<TaskEvent> = [];
  @State widgetFormInfo: string = '';
  @State widgetForms: Array<WidgetFormRecord> = [];
  @State widgetFavoritesInfo: string = '';

  @State historyLoading: boolean = false;
  @State historyError: string = '';
  @State historyNotice: string = '';
  @State historyTasks: Array<HistoryTaskMeta> = [];
  @State historyListNonce: number = 0;

  @State selectedTaskId: string = '';
  @State selectedTask: HistoryTaskMeta | null = null;
  @State selectedSteps: Array<HistoryStepMeta> = [];
  @State selectedStepIndex: number = 0;
  @State selectedStepPosition: number = 0;
  @State selectedStep: HistoryStepMeta | null = null;
  @State selectedStepPixelMap: image.PixelMap | null = null;
  @State selectedStepParamLines: Array<StepParamLine> = [];
  @State stepPixelMaps: Array<image.PixelMap | null> = [];
  @State stepPixelMapLoading: Array<boolean> = [];
  @State showTaskMeta: boolean = false;
  @State showStepMore: boolean = false;
  @State favoriteDialogVisible: boolean = false;
  @State favoriteDialogTaskId: string = '';
  @State favoriteDialogName: string = '';
  @State favoriteDialogError: string = '';
  @State private viewportHeight: number = 0;

  private stepPixelMapRequestSeq: number = 0;
  private stepImageCacheSeq: number = 0;
  private debugEventSequence: number = 0;
  private a2aStreamSequence: number = 0;
  private agentServiceClient: AgentControllerClient = new AgentControllerClient();

  aboutToAppear(): void {
    void this.loadSettings();
    void this.refreshHistory();
    void this.ensureWriteImageVideoPermission();
    void this.ensureMicrophonePermission();
    void this.ensureLocationPermission();
  }

  @Builder
  private settingsTab(): void {
    Scroll() {
      Column({ space: 12 }) {
        Text('配置')
          .fontSize(14)
          .fontFamily(FONT_DISPLAY)
          .fontColor(THEME_TEXT_PRIMARY)
          .fontWeight(FontWeight.Medium)

        TextInput({ placeholder: '云侧地址（http://ip:port）', text: this.cloudEndpoint })
          .height(32)
          .fontSize(12)
          .fontColor(THEME_TEXT_PRIMARY)
          .backgroundColor(THEME_CARD)
          .borderRadius(10)
          .border({ width: 1, color: THEME_BORDER })
          .padding({ left: 10, right: 10 })
          .onChange((value: string) => {
            this.cloudEndpoint = value;
          })

        Text('vNext 配置')
          .fontSize(12)
          .fontFamily(FONT_DISPLAY)
          .fontColor(THEME_TEXT_MUTED)

        TextInput({ placeholder: 'A2A Base URL（http://ip:port）', text: this.a2aBaseUrl })
          .height(32)
          .fontSize(12)
          .fontColor(THEME_TEXT_PRIMARY)
          .backgroundColor(THEME_CARD)
          .borderRadius(10)
          .border({ width: 1, color: THEME_BORDER })
          .padding({ left: 10, right: 10 })
          .onChange((value: string) => {
            this.a2aBaseUrl = value;
          })

        TextInput({ placeholder: 'MCP WS URL（ws://ip:port/ws）', text: this.mcpWsUrl })
          .height(32)
          .fontSize(12)
          .fontColor(THEME_TEXT_PRIMARY)
          .backgroundColor(THEME_CARD)
          .borderRadius(10)
          .border({ width: 1, color: THEME_BORDER })
          .padding({ left: 10, right: 10 })
          .onChange((value: string) => {
            this.mcpWsUrl = value;
          })

        Row({ space: 8 }) {
          TextInput({ placeholder: 'deviceId', text: this.deviceId })
            .layoutWeight(1)
            .height(32)
            .fontSize(12)
            .fontColor(THEME_TEXT_PRIMARY)
            .backgroundColor(THEME_CARD)
            .borderRadius(10)
            .border({ width: 1, color: THEME_BORDER })
            .padding({ left: 10, right: 10 })
            .onChange((value: string) => {
              this.deviceId = value;
            })

          TextInput({ placeholder: 'contextId', text: this.contextId })
            .layoutWeight(1)
            .height(32)
            .fontSize(12)
            .fontColor(THEME_TEXT_PRIMARY)
            .backgroundColor(THEME_CARD)
            .borderRadius(10)
            .border({ width: 1, color: THEME_BORDER })
            .padding({ left: 10, right: 10 })
            .onChange((value: string) => {
              this.contextId = value;
            })
        }

        Row({ space: 8 }) {
          TextInput({ placeholder: 'wait_ms (动作后等待)', text: this.waitMsText })
            .layoutWeight(1)
            .height(32)
            .fontSize(12)
            .fontColor(THEME_TEXT_PRIMARY)
            .backgroundColor(THEME_CARD)
            .borderRadius(10)
            .border({ width: 1, color: THEME_BORDER })
            .padding({ left: 10, right: 10 })
            .onChange((value: string) => {
              this.waitMsText = value;
            })

          Button(this.liveViewEnabled ? 'LiveView: 开' : 'LiveView: 关')
            .fontSize(12)
            .fontColor(this.liveViewEnabled ? THEME_ACCENT : THEME_TEXT_PRIMARY)
            .backgroundColor(this.liveViewEnabled ? THEME_ACCENT_SOFT : THEME_CARD)
            .borderRadius(14)
            .border({ width: 1, color: THEME_BORDER })
            .padding({ left: 12, right: 12, top: 6, bottom: 6 })
            .onClick(() => {
              this.liveViewEnabled = !this.liveViewEnabled;
            })
        }

        Row({ space: 8 }) {
          Button(this.historyEnabled ? '历史: 开' : '历史: 关')
            .fontSize(12)
            .fontColor(this.historyEnabled ? THEME_ACCENT : THEME_TEXT_PRIMARY)
            .backgroundColor(this.historyEnabled ? THEME_ACCENT_SOFT : THEME_CARD)
            .borderRadius(14)
            .border({ width: 1, color: THEME_BORDER })
            .padding({ left: 12, right: 12, top: 6, bottom: 6 })
            .onClick(() => {
              this.historyEnabled = !this.historyEnabled;
            })

          TextInput({ placeholder: '最多保留任务数', text: this.historyMaxTasksText })
            .layoutWeight(1)
            .height(32)
            .fontSize(12)
            .fontColor(THEME_TEXT_PRIMARY)
            .backgroundColor(THEME_CARD)
            .borderRadius(10)
            .border({ width: 1, color: THEME_BORDER })
            .padding({ left: 10, right: 10 })
            .onChange((value: string) => {
              this.historyMaxTasksText = value;
            })
        }

        Row({ space: 8 }) {
          Button('保存配置')
            .fontSize(12)
            .fontColor(Color.White)
            .backgroundColor(THEME_ACCENT)
            .borderRadius(14)
            .border({ width: 1, color: THEME_ACCENT })
            .padding({ left: 12, right: 12, top: 6, bottom: 6 })
            .onClick(() => {
              void this.saveSettings();
            })

          Button('刷新历史')
            .fontSize(12)
            .fontColor(THEME_TEXT_PRIMARY)
            .backgroundColor(THEME_CARD)
            .borderRadius(14)
            .border({ width: 1, color: THEME_BORDER })
            .padding({ left: 12, right: 12, top: 6, bottom: 6 })
            .onClick(() => {
              void this.refreshHistory();
            })
        }

        if (this.settingsInfo.length > 0) {
          Text(this.settingsInfo)
            .fontSize(11)
            .fontColor(THEME_TEXT_MUTED)
        }
      }
      .width('100%')
    }
    .scrollBar(BarState.Off)
    .layoutWeight(1)
  }

  @Builder
  private debugTab(): void {
    Scroll() {
      Column({ space: 12 }) {
        Text('调试')
          .fontSize(14)
          .fontFamily(FONT_DISPLAY)
          .fontColor(THEME_TEXT_PRIMARY)
          .fontWeight(FontWeight.Medium)

        Column({ space: 6 }) {
          Text('vNext 配置')
            .fontSize(12)
            .fontFamily(FONT_DISPLAY)
            .fontColor(THEME_TEXT_MUTED)

          Text(`A2A: ${this.a2aBaseUrl}`)
            .fontSize(11)
            .fontColor(THEME_TEXT_PRIMARY)

          Text(`MCP WS: ${this.mcpWsUrl}`)
            .fontSize(11)
            .fontColor(THEME_TEXT_PRIMARY)

          Text(`deviceId: ${this.deviceId} / contextId: ${this.contextId}`)
            .fontSize(11)
            .fontColor(THEME_TEXT_PRIMARY)
        }

        Column({ space: 8 }) {
          Text('桌面卡片')
            .fontSize(12)
            .fontFamily(FONT_DISPLAY)
            .fontColor(THEME_TEXT_MUTED)

          Row({ space: 8 }) {
            Button('刷新 formId')
              .fontSize(12)
              .fontColor(THEME_TEXT_PRIMARY)
              .backgroundColor(THEME_CARD)
              .borderRadius(14)
              .border({ width: 1, color: THEME_BORDER })
              .padding({ left: 12, right: 12, top: 6, bottom: 6 })
              .onClick(() => {
                void this.refreshWidgetForms();
              })

            Button('清空 formId')
              .fontSize(12)
              .fontColor(THEME_TEXT_PRIMARY)
              .backgroundColor(THEME_CARD)
              .borderRadius(14)
              .border({ width: 1, color: THEME_BORDER })
              .padding({ left: 12, right: 12, top: 6, bottom: 6 })
              .onClick(() => {
                void this.clearWidgetForms();
              })
          }

          Row({ space: 8 }) {
            Button('刷新卡片内容')
              .fontSize(12)
              .fontColor(THEME_TEXT_PRIMARY)
              .backgroundColor(THEME_CARD)
              .borderRadius(14)
              .border({ width: 1, color: THEME_BORDER })
              .padding({ left: 12, right: 12, top: 6, bottom: 6 })
              .onClick(() => {
                void this.refreshWidgetFavorites();
              })
          }

          if (this.widgetFormInfo.length > 0) {
            Text(this.widgetFormInfo)
              .fontSize(11)
              .fontColor(THEME_TEXT_MUTED)
          }

          if (this.widgetFavoritesInfo.length > 0) {
            Text(this.widgetFavoritesInfo)
              .fontSize(11)
              .fontColor(THEME_TEXT_MUTED)
          }

          if (this.widgetForms.length === 0) {
            Text('暂无 formId')
              .fontSize(11)
              .fontColor(THEME_TEXT_SUBTLE)
          } else {
            Column({ space: 4 }) {
              ForEach(this.widgetForms, (record: WidgetFormRecord) => {
                Text(this.formatWidgetFormLine(record))
                  .fontSize(10)
                  .fontColor(THEME_TEXT_MUTED)
              }, (record: WidgetFormRecord) => record.formId)
            }
            .padding({ left: 10, right: 10, top: 8, bottom: 8 })
            .backgroundColor(DETAIL_SECTION_BG)
            .borderRadius(10)
            .border({ width: 1, color: DETAIL_SECTION_BORDER })
          }
        }

        Column({ space: 8 }) {
          Text('MCP WS')
            .fontSize(12)
            .fontFamily(FONT_DISPLAY)
            .fontColor(THEME_TEXT_MUTED)

          Row({ space: 8 }) {
            Button('连接')
              .fontSize(12)
              .fontColor(THEME_TEXT_PRIMARY)
              .backgroundColor(THEME_CARD)
              .borderRadius(14)
              .border({ width: 1, color: THEME_BORDER })
              .padding({ left: 12, right: 12, top: 6, bottom: 6 })
              .onClick(() => {
                void this.connectMcp();
              })

            Button('断开')
              .fontSize(12)
              .fontColor(THEME_TEXT_PRIMARY)
              .backgroundColor(THEME_CARD)
              .borderRadius(14)
              .border({ width: 1, color: THEME_BORDER })
              .padding({ left: 12, right: 12, top: 6, bottom: 6 })
              .onClick(() => {
                void this.disconnectMcp();
              })

            Button('状态')
              .fontSize(12)
              .fontColor(THEME_TEXT_PRIMARY)
              .backgroundColor(THEME_CARD)
              .borderRadius(14)
              .border({ width: 1, color: THEME_BORDER })
              .padding({ left: 12, right: 12, top: 6, bottom: 6 })
              .onClick(() => {
                void this.refreshMcpStatus();
              })
          }

          if (this.mcpStatusInfo.length > 0) {
            Text(this.mcpStatusInfo)
              .fontSize(11)
              .fontColor(THEME_TEXT_MUTED)
          }
        }

        Column({ space: 8 }) {
          Text('MCP 日志')
            .fontSize(12)
            .fontFamily(FONT_DISPLAY)
            .fontColor(THEME_TEXT_MUTED)

          Row({ space: 8 }) {
            Button('刷新日志')
              .fontSize(12)
              .fontColor(THEME_TEXT_PRIMARY)
              .backgroundColor(THEME_CARD)
              .borderRadius(14)
              .border({ width: 1, color: THEME_BORDER })
              .padding({ left: 12, right: 12, top: 6, bottom: 6 })
              .onClick(() => {
                void this.refreshMcpLogs();
              })

            Button('清空日志')
              .fontSize(12)
              .fontColor(THEME_TEXT_PRIMARY)
              .backgroundColor(THEME_CARD)
              .borderRadius(14)
              .border({ width: 1, color: THEME_BORDER })
              .padding({ left: 12, right: 12, top: 6, bottom: 6 })
              .onClick(() => {
                void this.clearMcpLogs();
              })
          }

          if (this.mcpLogInfo.length > 0) {
            Text(this.mcpLogInfo)
              .fontSize(11)
              .fontColor(THEME_TEXT_MUTED)
          }

          if (this.mcpLogLines.length === 0) {
            Text('暂无日志')
              .fontSize(11)
              .fontColor(THEME_TEXT_SUBTLE)
          } else {
            Column({ space: 4 }) {
              ForEach(this.mcpLogLines, (line: string, index: number) => {
                Text(line)
                  .fontSize(10)
                  .fontColor(THEME_TEXT_MUTED)
              }, (line: string, index: number) => `${index}:${line}`)
            }
            .padding({ left: 10, right: 10, top: 8, bottom: 8 })
            .backgroundColor(DETAIL_SECTION_BG)
            .borderRadius(10)
            .border({ width: 1, color: DETAIL_SECTION_BORDER })
          }
        }

        Column({ space: 8 }) {
          Text('A2A Blocking')
            .fontSize(12)
            .fontFamily(FONT_DISPLAY)
            .fontColor(THEME_TEXT_MUTED)

          TextInput({ placeholder: '任务指令（prompt）', text: this.a2aPrompt })
            .height(36)
            .fontSize(12)
            .fontColor(THEME_TEXT_PRIMARY)
            .backgroundColor(THEME_CARD)
            .borderRadius(10)
            .border({ width: 1, color: THEME_BORDER })
            .padding({ left: 10, right: 10 })
            .onChange((value: string) => {
              this.a2aPrompt = value;
            })

          Row({ space: 8 }) {
            Button('Send')
              .fontSize(12)
              .fontColor(THEME_TEXT_PRIMARY)
              .backgroundColor(THEME_CARD)
              .borderRadius(14)
              .border({ width: 1, color: THEME_BORDER })
              .padding({ left: 12, right: 12, top: 6, bottom: 6 })
              .onClick(() => {
                void this.a2aSendBlocking();
              })

            Button('Resume')
              .fontSize(12)
              .fontColor(THEME_TEXT_PRIMARY)
              .backgroundColor(THEME_CARD)
              .borderRadius(14)
              .border({ width: 1, color: THEME_BORDER })
              .padding({ left: 12, right: 12, top: 6, bottom: 6 })
              .onClick(() => {
                void this.a2aResumeBlocking();
              })
          }

          TextInput({ placeholder: 'taskId（续跑）', text: this.a2aTaskId })
            .height(32)
            .fontSize(12)
            .fontColor(THEME_TEXT_PRIMARY)
            .backgroundColor(THEME_CARD)
            .borderRadius(10)
            .border({ width: 1, color: THEME_BORDER })
            .padding({ left: 10, right: 10 })
            .onChange((value: string) => {
              this.a2aTaskId = value;
            })

          TextInput({ placeholder: '用户回复（reply）', text: this.a2aReply })
            .height(32)
            .fontSize(12)
            .fontColor(THEME_TEXT_PRIMARY)
            .backgroundColor(THEME_CARD)
            .borderRadius(10)
            .border({ width: 1, color: THEME_BORDER })
            .padding({ left: 10, right: 10 })
            .onChange((value: string) => {
              this.a2aReply = value;
            })

          if (this.a2aResultInfo.length > 0) {
            Text(this.a2aResultInfo)
              .fontSize(11)
              .fontColor(THEME_TEXT_MUTED)
          }

          if (this.a2aResult.length > 0) {
            Text(this.formatA2AResult(this.a2aResult))
              .fontSize(10)
              .fontColor(THEME_TEXT_MUTED)
          }
        }

        Column({ space: 8 }) {
          Text('A2A Stream')
            .fontSize(12)
            .fontFamily(FONT_DISPLAY)
            .fontColor(THEME_TEXT_MUTED)

          TextInput({ placeholder: 'stream prompt', text: this.a2aStreamPrompt })
            .height(36)
            .fontSize(12)
            .fontColor(THEME_TEXT_PRIMARY)
            .backgroundColor(THEME_CARD)
            .borderRadius(10)
            .border({ width: 1, color: THEME_BORDER })
            .padding({ left: 10, right: 10 })
            .onChange((value: string) => {
              this.a2aStreamPrompt = value;
            })

          TextInput({ placeholder: 'stream contextId（可选）', text: this.a2aStreamContextId })
            .height(32)
            .fontSize(12)
            .fontColor(THEME_TEXT_PRIMARY)
            .backgroundColor(THEME_CARD)
            .borderRadius(10)
            .border({ width: 1, color: THEME_BORDER })
            .padding({ left: 10, right: 10 })
            .onChange((value: string) => {
              this.a2aStreamContextId = value;
            })

          Row({ space: 8 }) {
            Button('Start Stream')
              .fontSize(12)
              .fontColor(THEME_TEXT_PRIMARY)
              .backgroundColor(THEME_CARD)
              .borderRadius(14)
              .border({ width: 1, color: THEME_BORDER })
              .padding({ left: 12, right: 12, top: 6, bottom: 6 })
              .onClick(() => {
                void this.startA2AStream();
              })

            Button('Stop Stream')
              .fontSize(12)
              .fontColor(THEME_TEXT_PRIMARY)
              .backgroundColor(THEME_CARD)
              .borderRadius(14)
              .border({ width: 1, color: THEME_BORDER })
              .padding({ left: 12, right: 12, top: 6, bottom: 6 })
              .onClick(() => {
                void this.stopA2AStream();
              })
          }

          Row({ space: 8 }) {
            Button('拉取A2A事件')
              .fontSize(12)
              .fontColor(THEME_TEXT_PRIMARY)
              .backgroundColor(THEME_CARD)
              .borderRadius(14)
              .border({ width: 1, color: THEME_BORDER })
              .padding({ left: 12, right: 12, top: 6, bottom: 6 })
              .onClick(() => {
                void this.pullA2AStreamEvents();
              })

            Button('清空A2A事件')
              .fontSize(12)
              .fontColor(THEME_TEXT_PRIMARY)
              .backgroundColor(THEME_CARD)
              .borderRadius(14)
              .border({ width: 1, color: THEME_BORDER })
              .padding({ left: 12, right: 12, top: 6, bottom: 6 })
              .onClick(() => {
                this.clearA2AStreamEvents();
              })
          }

          if (this.a2aStreamInfo.length > 0) {
            Text(this.a2aStreamInfo)
              .fontSize(11)
              .fontColor(THEME_TEXT_MUTED)
          }

          if (this.a2aStreamEvents.length === 0) {
            Text('暂无A2A事件')
              .fontSize(11)
              .fontColor(THEME_TEXT_SUBTLE)
          } else {
            Column({ space: 6 }) {
              ForEach(this.a2aStreamEvents, (event: TaskEvent) => {
                Column({ space: 4 }) {
                  Text(this.formatEventLine(event))
                    .fontSize(11)
                    .fontColor(THEME_TEXT_PRIMARY)

                  if (event.payloadJson.length > 0) {
                    Text(this.formatEventPayload(event.payloadJson))
                      .fontSize(10)
                      .fontColor(THEME_TEXT_MUTED)
                  }
                }
                .padding({ left: 10, right: 10, top: 8, bottom: 8 })
                .backgroundColor(DETAIL_SECTION_BG)
                .borderRadius(10)
                .border({ width: 1, color: DETAIL_SECTION_BORDER })
              }, (event: TaskEvent) => `a2a_${event.seq}`)
            }
          }
        }

        Column({ space: 8 }) {
          Text('事件队列')
            .fontSize(12)
            .fontFamily(FONT_DISPLAY)
            .fontColor(THEME_TEXT_MUTED)

          Row({ space: 8 }) {
            Button('拉取事件')
              .fontSize(12)
              .fontColor(THEME_TEXT_PRIMARY)
              .backgroundColor(THEME_CARD)
              .borderRadius(14)
              .border({ width: 1, color: THEME_BORDER })
              .padding({ left: 12, right: 12, top: 6, bottom: 6 })
              .onClick(() => {
                void this.pullDebugEvents();
              })

            Button('清空事件')
              .fontSize(12)
              .fontColor(THEME_TEXT_PRIMARY)
              .backgroundColor(THEME_CARD)
              .borderRadius(14)
              .border({ width: 1, color: THEME_BORDER })
              .padding({ left: 12, right: 12, top: 6, bottom: 6 })
              .onClick(() => {
                this.clearDebugEvents();
              })
          }

          if (this.debugEventInfo.length > 0) {
            Text(this.debugEventInfo)
              .fontSize(11)
              .fontColor(THEME_TEXT_MUTED)
          }

          if (this.debugEvents.length === 0) {
            Text('暂无事件')
              .fontSize(11)
              .fontColor(THEME_TEXT_SUBTLE)
          } else {
            Column({ space: 6 }) {
              ForEach(this.debugEvents, (event: TaskEvent) => {
                Column({ space: 4 }) {
                  Text(this.formatEventLine(event))
                    .fontSize(11)
                    .fontColor(THEME_TEXT_PRIMARY)

                  if (event.payloadJson.length > 0) {
                    Text(this.formatEventPayload(event.payloadJson))
                      .fontSize(10)
                      .fontColor(THEME_TEXT_MUTED)
                  }
                }
                .padding({ left: 10, right: 10, top: 8, bottom: 8 })
                .backgroundColor(DETAIL_SECTION_BG)
                .borderRadius(10)
                .border({ width: 1, color: DETAIL_SECTION_BORDER })
              }, (event: TaskEvent) => String(event.seq))
            }
          }
        }

        Column({ space: 8 }) {
          Text('工具模拟')
            .fontSize(12)
            .fontFamily(FONT_DISPLAY)
            .fontColor(THEME_TEXT_MUTED)

          Row({ space: 8 }) {
            TextInput({ placeholder: 'tool name', text: this.debugToolName })
              .layoutWeight(1)
              .height(32)
              .fontSize(12)
              .fontColor(THEME_TEXT_PRIMARY)
              .backgroundColor(THEME_CARD)
              .borderRadius(10)
              .border({ width: 1, color: THEME_BORDER })
              .padding({ left: 10, right: 10 })
              .onChange((value: string) => {
                this.debugToolName = value;
              })

            Button('执行')
              .fontSize(12)
              .fontColor(THEME_TEXT_PRIMARY)
              .backgroundColor(THEME_CARD)
              .borderRadius(14)
              .border({ width: 1, color: THEME_BORDER })
              .padding({ left: 12, right: 12, top: 6, bottom: 6 })
              .onClick(() => {
                void this.simulateToolCall();
              })
          }

          TextInput({ placeholder: 'tool arguments JSON', text: this.debugToolArgs })
            .height(32)
            .fontSize(12)
            .fontColor(THEME_TEXT_PRIMARY)
            .backgroundColor(THEME_CARD)
            .borderRadius(10)
            .border({ width: 1, color: THEME_BORDER })
            .padding({ left: 10, right: 10 })
            .onChange((value: string) => {
              this.debugToolArgs = value;
            })

          if (this.debugToolResult.length > 0) {
            Text(this.formatDebugToolResult(this.debugToolResult))
              .fontSize(11)
              .fontColor(THEME_TEXT_MUTED)
          }
        }

        Row({ space: 8 }) {
          Button('获取壁纸')
            .fontSize(12)
            .fontColor(THEME_TEXT_PRIMARY)
            .backgroundColor(THEME_CARD)
            .borderRadius(14)
            .border({ width: 1, color: THEME_BORDER })
            .padding({ left: 12, right: 12, top: 6, bottom: 6 })
            .onClick(() => {
              void this.loadWallpaper();
            })

          Button('前台应用')
            .fontSize(12)
            .fontColor(THEME_TEXT_PRIMARY)
            .backgroundColor(THEME_CARD)
            .borderRadius(14)
            .border({ width: 1, color: THEME_BORDER })
            .padding({ left: 12, right: 12, top: 6, bottom: 6 })
            .onClick(() => {
              void this.loadForegroundApp();
            })

          Button('获取定位')
            .fontSize(12)
            .fontColor(THEME_TEXT_PRIMARY)
            .backgroundColor(THEME_CARD)
            .borderRadius(14)
            .border({ width: 1, color: THEME_BORDER })
            .padding({ left: 12, right: 12, top: 6, bottom: 6 })
            .onClick(() => {
              void this.loadLocation();
            })
        }

        Row({ space: 8 }) {
          Button('打开 AgentSpaceViewer')
            .fontSize(12)
            .fontColor(THEME_TEXT_PRIMARY)
            .backgroundColor(THEME_CARD)
            .borderRadius(14)
            .border({ width: 1, color: THEME_BORDER })
            .padding({ left: 12, right: 12, top: 6, bottom: 6 })
            .onClick(() => {
              void this.openAgentSpaceViewer();
            })

          Button('初始化 AgentSpace')
            .fontSize(12)
            .fontColor(THEME_TEXT_PRIMARY)
            .backgroundColor(THEME_CARD)
            .borderRadius(14)
            .border({ width: 1, color: THEME_BORDER })
            .padding({ left: 12, right: 12, top: 6, bottom: 6 })
            .onClick(() => {
              void this.initAgentSpace();
            })
        }

        Row({ space: 8 }) {
          TextInput({ placeholder: '应用名称（如 微信 / com.xxx 或 com.xxx/EntryAbility）', text: this.agentSpaceAppName })
            .layoutWeight(1)
            .height(32)
            .fontSize(12)
            .fontColor(THEME_TEXT_PRIMARY)
            .backgroundColor(THEME_CARD)
            .borderRadius(10)
            .border({ width: 1, color: THEME_BORDER })
            .padding({ left: 10, right: 10 })
            .onChange((value: string) => {
              this.agentSpaceAppName = value;
            })

          Button('打开应用')
            .fontSize(12)
            .fontColor(THEME_TEXT_PRIMARY)
            .backgroundColor(THEME_CARD)
            .borderRadius(14)
            .border({ width: 1, color: THEME_BORDER })
            .padding({ left: 12, right: 12, top: 6, bottom: 6 })
            .onClick(() => {
              void this.openAgentSpaceApp();
            })
        }

        Row({ space: 8 }) {
          Button('获取 UI 树列表')
            .fontSize(12)
            .fontColor(THEME_TEXT_PRIMARY)
            .backgroundColor(THEME_CARD)
            .borderRadius(14)
            .border({ width: 1, color: THEME_BORDER })
            .padding({ left: 12, right: 12, top: 6, bottom: 6 })
            .onClick(() => {
              void this.debugGetA11yTreeList();
            })
        }

        Row({ space: 8 }) {
          Button('唤出全局浮窗')
            .fontSize(12)
            .fontColor(THEME_TEXT_PRIMARY)
            .backgroundColor(THEME_CARD)
            .borderRadius(14)
            .border({ width: 1, color: THEME_BORDER })
            .padding({ left: 12, right: 12, top: 6, bottom: 6 })
            .onClick(() => {
              void this.showGlobalFloatWindow();
            })
        }

        if (this.wallpaperInfo.length > 0) {
          Text(this.wallpaperInfo)
            .fontSize(11)
            .fontColor(THEME_TEXT_MUTED)
        }

        if (this.foregroundAppInfo.length > 0) {
          Text(this.foregroundAppInfo)
            .fontSize(11)
            .fontColor(THEME_TEXT_MUTED)
        }

        if (this.locationInfo.length > 0) {
          Text(this.locationInfo)
            .fontSize(11)
            .fontColor(THEME_TEXT_MUTED)
        }

        if (this.agentSpaceInfo.length > 0) {
          Text(this.agentSpaceInfo)
            .fontSize(11)
            .fontColor(THEME_TEXT_MUTED)
        }

        if (this.a11yTreeListInfo.length > 0) {
          Text(this.a11yTreeListInfo)
            .fontSize(11)
            .fontColor(THEME_TEXT_MUTED)
        }

        if (this.wallpaperPixelMap) {
          Image(this.wallpaperPixelMap)
            .width('100%')
            .height(280)
            .borderRadius(12)
            .backgroundColor(THEME_CARD)
            .border({ width: 1, color: THEME_BORDER })
            .objectFit(ImageFit.Contain)
        }
      }
      .width('100%')
    }
    .scrollBar(BarState.Off)
    .layoutWeight(1)
  }

  @Builder
  private historyTab(): void {
    Scroll() {
      Column({ space: 12 }) {
        Column({ space: 12 }) {
          if (this.historyLoading) {
            Text('加载中...')
              .fontSize(11)
              .fontColor(THEME_TEXT_MUTED)
          }

          if (this.historyError.length > 0) {
            Text(this.historyError)
              .fontSize(11)
              .fontColor(THEME_ERROR)
          }

          if (this.historyNotice.length > 0) {
            Text(this.historyNotice)
              .fontSize(11)
              .fontColor(THEME_TEXT_MUTED)
          }

          if (this.selectedTask && this.selectedTaskId.length > 0) {
            this.taskDetailView();
          } else {
            this.taskListView();
          }

          this.navSpacer();
        }
        .width('100%')
        .padding({ left: 0, right: 0, top: 12, bottom: 0 })
      }
      .width('100%')
    }
    .scrollBar(BarState.Off)
    .layoutWeight(1)
  }

  @Builder
  private taskListView(): void {
    if (this.historyTasks.length === 0) {
      Text('暂无历史任务')
        .fontSize(11)
        .fontColor(THEME_TEXT_MUTED)
    } else {
      List({ space: 14 }) {
        ForEach(this.historyTasks, (task: HistoryTaskMeta) => {
          this.taskCardItem(task);
        }, (task: HistoryTaskMeta) => `${task.taskId}:${task.skill_name}:${this.historyListNonce}`)
      }
      .width('100%')
    }
  }

  @Builder
  private taskDetailView(): void {
    if (this.selectedTask) {
      Column({ space: 14 }) {
        this.taskSummaryCard(this.selectedTask);

        if (this.selectedSteps.length > 0) {
          this.stepSwiperView();
        } else {
          Text('暂无步骤')
            .fontSize(11)
            .fontColor(THEME_TEXT_MUTED)
        }
      }
      .width('100%')
      .padding({ left: 15, right: 15, top: 10 })
    }
  }

  @Builder
  private taskSummaryCard(task: HistoryTaskMeta): void {
    Column({ space: 12 }) {
      Row({ space: 8 }) {
        this.detailTag(this.resolveTaskStatusLabel(task.status), this.resolveStatusTagText(task.status), this.resolveStatusTagBg(task.status))
        this.detailTag(this.resolveTaskDuration(task), THEME_TEXT_PRIMARY, DETAIL_TAG_BG)
        Row()
          .layoutWeight(1)
      }

      Text(task.taskText)
        .fontSize(16)
        .fontColor(THEME_TEXT_PRIMARY)
        .fontWeight(FontWeight.Bold)
        .maxLines(3)
        .textOverflow({ overflow: TextOverflow.Ellipsis })

      this.statusCallout(
        '任务结果',
        this.resolveTaskResult(task),
        this.resolveTaskBackground(task.status),
        this.resolveTaskBorderColor(task.status),
        this.isTaskFinished(task.status) ? THEME_TEXT_PRIMARY : THEME_ERROR
      )

      Row({ space: 8 }) {
        this.detailTag(`步骤 ${task.stepCount}`, THEME_TEXT_MUTED, DETAIL_SECTION_BG)
        this.detailTag(`wait ${task.waitAfterActionMs}ms`, THEME_TEXT_MUTED, DETAIL_SECTION_BG)
      }

      Row({ space: 8 }) {
        Button('回放任务')
          .fontSize(12)
          .fontColor(Color.White)
          .backgroundColor(THEME_ACCENT)
          .borderRadius(14)
          .border({ width: 1, color: THEME_ACCENT })
          .padding({ left: 12, right: 12, top: 6, bottom: 6 })
          .onClick(() => {
            void this.replayHistoryTask(task.taskId);
          })

        Row()
          .layoutWeight(1)
      }

      Row() {
        Row()
          .layoutWeight(1)

        Text(this.showTaskMeta ? '收起信息' : '更多信息')
          .fontSize(11)
          .fontColor(THEME_ACCENT)
          .fontWeight(FontWeight.Medium)
      }
      .width('100%')
      .onClick(() => {
        this.showTaskMeta = !this.showTaskMeta;
      })

      if (this.showTaskMeta) {
        Column({ space: 10 }) {
          Row({ space: 12 }) {
            Column({ space: 4 }) {
              Text('开始时间')
                .fontSize(10)
                .fontColor(THEME_TEXT_MUTED)
              Text(this.formatTime(task.startAtMs))
                .fontSize(11)
                .fontColor(THEME_TEXT_PRIMARY)
            }
            .layoutWeight(1)

            Column({ space: 4 }) {
              Text('结束时间')
                .fontSize(10)
                .fontColor(THEME_TEXT_MUTED)
              Text(task.endAtMs > 0 ? this.formatTime(task.endAtMs) : '-')
                .fontSize(11)
                .fontColor(THEME_TEXT_PRIMARY)
            }
            .layoutWeight(1)
          }

          Column({ space: 6 }) {
            this.detailRow('任务ID', task.taskId)
            this.detailRow('Session', task.sessionId)
            this.detailRow('云端', task.cloudEndpoint)
            this.detailRow('wait_ms', String(task.waitAfterActionMs))
          }
          .padding(12)
          .borderRadius(DETAIL_SECTION_RADIUS)
          .backgroundColor(DETAIL_SECTION_BG)
          .border({ width: 1, color: DETAIL_SECTION_BORDER })
        }
      }
    }
    .width('100%')
    .padding(14)
    .borderRadius(DETAIL_CARD_RADIUS)
    .backgroundColor(DETAIL_CARD_BG)
    .border({ width: 1, color: DETAIL_CARD_BORDER })
    .shadow({ radius: 12, color: 'rgba(0, 0, 0, 0.06)', offsetX: 0, offsetY: 6 })
  }

  @Builder
  private detailRow(label: string, value: string, valueColor: string = THEME_TEXT_PRIMARY): void {
    Row({ space: 8 }) {
      Text(label)
        .fontSize(DETAIL_TEXT_SIZE)
        .fontColor(THEME_TEXT_MUTED)
        .width(72)

      Text(value.trim().length > 0 ? value : '-')
        .fontSize(DETAIL_TEXT_SIZE)
        .fontColor(valueColor)
        .layoutWeight(1)
        .lineHeight(16)
    }
  }

  @Builder
  private stepSwiperView(): void {
    Swiper() {
      ForEach(this.selectedSteps, (step: HistoryStepMeta, index: number) => {
        this.stepScreenshotSlide(step, index);
      }, (step: HistoryStepMeta) => `${step.taskId}:${step.stepIndex}`)
    }
    .width('100%')
    .loop(false)
    .indicator(true)
    .index(this.resolveStepSwiperIndex())
    .onChange((index: number) => {
      this.onStepChange(index);
    })
    .height(STEP_SWIPER_HEIGHT)
  }

  @Builder
  private stepScreenshotSlide(step: HistoryStepMeta, position: number): void {
    Column({ space: 12 }) {
      Row({ space: 8 }) {
        this.detailTag(`步骤 ${position + 1}/${this.selectedSteps.length}`, THEME_TEXT_PRIMARY, DETAIL_TAG_BG)
        this.detailTag(this.resolveActionDisplayName(step.actionName), DETAIL_TAG_TEXT, DETAIL_TAG_BG)
        this.detailTag(
          this.resolveStepStatusLabel(step.resultStatus),
          this.resolveStepStatusText(step.resultStatus),
          this.resolveStepStatusBg(step.resultStatus)
        )
        Row()
          .layoutWeight(1)
        Text(this.formatTimeShort(step.tsMs))
          .fontSize(10)
          .fontColor(DETAIL_HINT_TEXT)
          .maxLines(1)
          .textOverflow({ overflow: TextOverflow.Ellipsis })
      }

      Column() {
        this.stepScreenshot(step, position);
      }
      .width('100%')
      .height(STEP_IMAGE_HEIGHT)
      .borderRadius(DETAIL_SECTION_RADIUS)
      .clip(true)
      .backgroundColor(DETAIL_SECTION_BG)
      .border({ width: 1, color: DETAIL_SECTION_BORDER })

      Text(step.description.trim().length > 0 ? step.description : '暂无步骤说明')
        .fontSize(13)
        .fontColor(THEME_TEXT_PRIMARY)
        .lineHeight(20)
        .maxLines(2)
        .textOverflow({ overflow: TextOverflow.Ellipsis })

      if ((step.uiTargetText ?? '').trim().length > 0) {
        Text(`命中：${(step.uiTargetText ?? '').trim()}`)
          .fontSize(11)
          .fontColor(THEME_TEXT_MUTED)
          .lineHeight(16)
          .maxLines(2)
          .textOverflow({ overflow: TextOverflow.Ellipsis })
      }

      Text(this.resolveStepFooterText(step))
        .fontSize(11)
        .fontColor(THEME_TEXT_MUTED)
        .lineHeight(16)
        .maxLines(1)
        .textOverflow({ overflow: TextOverflow.Ellipsis })
    }
    .width('100%')
    .padding(14)
    .backgroundColor(DETAIL_CARD_BG)
    .borderRadius(DETAIL_CARD_RADIUS)
    .border({ width: 1, color: DETAIL_CARD_BORDER })
    .shadow({ radius: 14, color: 'rgba(0, 0, 0, 0.08)', offsetX: 0, offsetY: 8 })
    .onAppear(() => {
      this.ensureStepPixelMapLoaded(step.taskId, step.stepIndex, position);
    })
  }

  @Builder
  private stepScreenshotCard(step: HistoryStepMeta, position: number): void {
    Column() {
      Stack() {
        this.stepScreenshot(step, position);

        Column() {
          Row({ space: 8 }) {
            this.detailTag(`步骤 ${position + 1}/${this.selectedSteps.length}`, THEME_TEXT_PRIMARY, 'rgba(255, 255, 255, 0.86)')
            Row()
              .layoutWeight(1)
            this.detailTag(
              this.resolveStepStatusLabel(step.resultStatus),
              this.resolveStepStatusText(step.resultStatus),
              this.resolveStepStatusBg(step.resultStatus)
            )
          }
          .width('100%')

          Row({ space: 8 }) {
            this.detailTag(this.resolveActionDisplayName(step.actionName), THEME_TEXT_PRIMARY, 'rgba(255, 255, 255, 0.86)')
            Row()
              .layoutWeight(1)
          }
          .width('100%')
        }
        .width('100%')
        .height('100%')
        .padding(12)
        .justifyContent(FlexAlign.SpaceBetween)
      }
      .width('100%')
      .height('100%')
      .borderRadius(DETAIL_CARD_RADIUS)
      .clip(true)
    }
    .width('100%')
    .height(STEP_IMAGE_HEIGHT)
    .backgroundColor(DETAIL_SECTION_BG)
    .borderRadius(DETAIL_CARD_RADIUS)
    .border({ width: 1, color: DETAIL_CARD_BORDER })
    .shadow({ radius: 12, color: 'rgba(0, 0, 0, 0.08)', offsetX: 0, offsetY: 6 })
  }

  @Builder
  private stepScreenshot(step: HistoryStepMeta, position: number): void {
    if (this.resolveStepPixelMap(position) !== null) {
      Image(this.resolveStepPixelMap(position) as image.PixelMap)
        .width('100%')
        .height('100%')
        .backgroundColor(THEME_CARD)
        .objectFit(ImageFit.Contain)
    } else {
      Row() {
        Text(this.resolveStepPixelMapLoading(position) ? '正在加载截图…' : '暂无截图')
          .fontSize(11)
          .fontColor(THEME_TEXT_MUTED)
      }
      .width('100%')
      .height('100%')
      .backgroundColor(DETAIL_SECTION_BG)
      .justifyContent(FlexAlign.Center)
      .alignItems(VerticalAlign.Center)
    }
  }

  @Builder
  private stepDetailPanel(step: HistoryStepMeta): void {
    Column({ space: 12 }) {
      Row({ space: 8 }) {
        this.detailTag(`步骤 ${this.resolveStepSwiperIndex() + 1}/${this.selectedSteps.length}`, THEME_TEXT_PRIMARY, DETAIL_TAG_BG)
        this.detailTag(this.resolveActionDisplayName(step.actionName), DETAIL_TAG_TEXT, DETAIL_TAG_BG)
        this.detailTag(
          this.resolveStepStatusLabel(step.resultStatus),
          this.resolveStepStatusText(step.resultStatus),
          this.resolveStepStatusBg(step.resultStatus)
        )
        Row()
          .layoutWeight(1)
        Text(this.formatTimeShort(step.tsMs))
          .fontSize(10)
          .fontColor(DETAIL_HINT_TEXT)
          .maxLines(1)
          .textOverflow({ overflow: TextOverflow.Ellipsis })
      }

      Text(step.description.trim().length > 0 ? step.description : '暂无步骤说明')
        .fontSize(13)
        .fontColor(THEME_TEXT_PRIMARY)
        .lineHeight(20)

      if ((step.uiTargetText ?? '').trim().length > 0) {
        Column({ space: 6 }) {
          Text('命中文本')
            .fontSize(DETAIL_TITLE_SIZE)
            .fontColor(THEME_TEXT_PRIMARY)
            .fontWeight(FontWeight.Medium)

          Text((step.uiTargetText ?? '').trim())
            .fontSize(DETAIL_TEXT_SIZE)
            .fontColor(THEME_TEXT_PRIMARY)
            .lineHeight(18)
        }
        .padding(12)
        .borderRadius(DETAIL_SECTION_RADIUS)
        .backgroundColor(DETAIL_SECTION_BG)
        .border({ width: 1, color: DETAIL_SECTION_BORDER })
      }

      if (!this.showStepMore && this.selectedStepParamLines.length > 0) {
        Column({ space: 6 }) {
          Text('关键参数')
            .fontSize(DETAIL_TITLE_SIZE)
            .fontColor(THEME_TEXT_PRIMARY)
            .fontWeight(FontWeight.Medium)

          ForEach(this.resolveStepParamPreviewLines(), (line: StepParamLine) => {
            this.detailRow(line.label, line.value)
          }, (line: StepParamLine) => line.label)
        }
        .padding(12)
        .borderRadius(DETAIL_SECTION_RADIUS)
        .backgroundColor(DETAIL_SECTION_BG)
        .border({ width: 1, color: DETAIL_SECTION_BORDER })
      }

      Row() {
        Row()
          .layoutWeight(1)
        Text(this.showStepMore ? '收起更多信息' : '查看更多信息')
          .fontSize(11)
          .fontColor(THEME_ACCENT)
          .fontWeight(FontWeight.Medium)
      }
      .width('100%')
      .onClick(() => {
        this.showStepMore = !this.showStepMore;
      })

      if (this.showStepMore) {
        if (this.selectedStepParamLines.length > 0) {
          Column({ space: 6 }) {
            Text('动作参数')
              .fontSize(DETAIL_TITLE_SIZE)
              .fontColor(THEME_TEXT_PRIMARY)
              .fontWeight(FontWeight.Medium)

            ForEach(this.selectedStepParamLines, (line: StepParamLine) => {
              this.detailRow(line.label, line.value)
            }, (line: StepParamLine) => `${line.label}:${line.value}`)
          }
          .padding(12)
          .borderRadius(DETAIL_SECTION_RADIUS)
          .backgroundColor(DETAIL_SECTION_BG)
          .border({ width: 1, color: DETAIL_SECTION_BORDER })
        }

        Column({ space: 6 }) {
          this.detailRow('截图格式', step.image.format)
          this.detailRow('截图大小', this.formatBytes(step.image.bytes))
        }
        .padding(12)
        .borderRadius(DETAIL_SECTION_RADIUS)
        .backgroundColor(DETAIL_SECTION_BG)
        .border({ width: 1, color: DETAIL_SECTION_BORDER })
      }
    }
    .width('100%')
  }

  @Builder
  private statusCallout(title: string, content: string, background: string, borderColor: string, textColor: string): void {
    Column({ space: 6 }) {
      Text(title)
        .fontSize(DETAIL_TITLE_SIZE)
        .fontColor(THEME_TEXT_PRIMARY)
        .fontWeight(FontWeight.Medium)

      Text(content.trim().length > 0 ? content : '-')
        .fontSize(DETAIL_TEXT_SIZE)
        .fontColor(textColor)
        .lineHeight(18)
        .maxLines(6)
        .textOverflow({ overflow: TextOverflow.Ellipsis })
    }
    .padding(12)
    .borderRadius(DETAIL_SECTION_RADIUS)
    .backgroundColor(background)
    .border({ width: 1, color: borderColor })
  }

  @Builder
  private detailSection(
    title: string,
    content: string,
    contentColor: string = THEME_TEXT_PRIMARY,
    maxLines: number = 10
  ): void {
    Column({ space: 6 }) {
      Text(title)
        .fontSize(DETAIL_TITLE_SIZE)
        .fontColor(THEME_TEXT_PRIMARY)
        .fontWeight(FontWeight.Medium)

      Text(content.trim().length > 0 ? content : '-')
        .fontSize(DETAIL_TEXT_SIZE)
        .fontColor(contentColor)
        .maxLines(maxLines)
        .textOverflow({ overflow: TextOverflow.Ellipsis })
        .lineHeight(18)
    }
    .padding(12)
    .borderRadius(DETAIL_SECTION_RADIUS)
    .backgroundColor(DETAIL_SECTION_BG)
    .border({ width: 1, color: DETAIL_SECTION_BORDER })
  }

  @Builder
  private navSpacer(): void {
    Row()
      .width('100%')
      .height(NAV_HEIGHT + NAV_SAFE_SPACER)
      .backgroundColor('transparent')
  }

  @Builder
  private stepListView(): void {
    if (this.selectedSteps.length === 0) {
      Text('暂无步骤')
        .fontSize(11)
        .fontColor(THEME_TEXT_MUTED)
    } else {
      List({ space: 8 }) {
        ForEach(this.selectedSteps, (step: HistoryStepMeta) => {
          ListItem() {
            Column({ space: 6 }) {
              Row() {
                Text(`#${step.stepIndex}  ${step.actionName}`)
                  .fontSize(12)
                  .fontColor(THEME_TEXT_PRIMARY)
                  .layoutWeight(1)

                Text(step.resultStatus.length > 0 ? step.resultStatus : '-')
                  .fontSize(11)
                  .fontColor(THEME_TEXT_MUTED)
              }

              if (step.description.trim().length > 0) {
                Text(step.description)
                  .fontSize(11)
                  .fontColor(THEME_TEXT_MUTED)
                  .maxLines(2)
                  .textOverflow({ overflow: TextOverflow.Ellipsis })
              }

              if ((step.uiTargetText ?? '').trim().length > 0) {
                Text(`命中：${(step.uiTargetText ?? '').trim()}`)
                  .fontSize(10)
                  .fontColor(THEME_TEXT_SUBTLE)
                  .maxLines(1)
                  .textOverflow({ overflow: TextOverflow.Ellipsis })
              }
            }
            .padding(12)
            .borderRadius(12)
            .backgroundColor(this.selectedStepIndex === step.stepIndex ? THEME_NAV_ACTIVE_BG : THEME_CARD)
            .onClick(() => {
              void this.selectStep(step);
            })
          }
        }, (step: HistoryStepMeta) => `${step.taskId}:${step.stepIndex}`)
      }
      .width('100%')
    }
  }

  @Builder
  private stepDetailView(): void {
    if (this.selectedStep) {
      Column({ space: 6 }) {
        Text(`Step #${this.selectedStep.stepIndex}`)
          .fontSize(12)
          .fontColor(THEME_TEXT_PRIMARY)
          .fontWeight(FontWeight.Medium)

        if (this.selectedStep.error.trim().length > 0) {
          Text(this.selectedStep.error)
            .fontSize(11)
            .fontColor(THEME_ERROR)
        }

        if (this.selectedStep.output.trim().length > 0) {
          Text(this.selectedStep.output)
            .fontSize(11)
            .fontColor(THEME_TEXT_MUTED)
        }
      }
      .padding(12)
      .borderRadius(12)
      .backgroundColor(THEME_CARD)
      .width('100%')
    }
  }

  @Builder
  private pageHeader(): void {
    Column({ space: 4 }) {
      Text(this.resolveTabTitle())
        .fontSize(20)
        .fontFamily(FONT_DISPLAY)
        .fontColor(THEME_TEXT_PRIMARY)
        .fontWeight(FontWeight.Medium)

      Text('Assistant Console')
        .fontSize(11)
        .fontColor(THEME_TEXT_SUBTLE)
        .letterSpacing(0.4)
    }
    .width('100%')
  }

  @Builder
  private activeTabContent(): void {
    if (this.activeTabIndex === 0) {
      this.taskTabContent();
    } else if (this.activeTabIndex === 1) {
      this.debugTab();
    } else {
      this.settingsTab();
    }
  }

  @Builder
  private taskTabContent(): void {
    Column({ space: 0 }) {
      this.taskTopBar();
      this.historyTab();
    }
    .width('100%')
    .layoutWeight(1)
  }

  @Builder
  private navItem(label: string, symbolName: NavSymbolName, index: number): void {
    Column({ space: 6 }) {
      this.navSymbol(symbolName, this.isTabActive(index));

      Text(label)
        .fontSize(11)
        .fontFamily(FONT_DISPLAY)
        .fontColor(this.isTabActive(index) ? THEME_ACCENT : THEME_TEXT_SUBTLE)
        .fontWeight(this.isTabActive(index) ? FontWeight.Bold : FontWeight.Medium)
    }
    .height('100%')
    .layoutWeight(1)
    .alignItems(HorizontalAlign.Center)
    .justifyContent(FlexAlign.Start)
    .padding({ top: 4, bottom: 16 })
    .backgroundColor('transparent')
    .onClick(() => {
      this.activeTabIndex = index;
    })
  }

  @Builder
  private bottomNav(): void {
    Row() {
      this.navItem('Task', 'house_fill', 0);
      this.navItem('Debug', 'bolt_filled_on_circle', 1);
      this.navItem('Setting', 'gearshape', 2);
    }
    .width('100%')
    .height(NAV_HEIGHT)
    .justifyContent(FlexAlign.SpaceEvenly)
    .backgroundColor('rgba(255, 255, 255, 0.72)')
    .backdropBlur(NAV_BLUR_RADIUS)
    .border({ width: 1, color: THEME_BORDER })
    .clip(true)
    .position({ x: 0, y: this.resolveNavY() })
  }

  private resolveTabTitle(): string {
    if (this.activeTabIndex === 0) {
      return 'Task';
    }
    if (this.activeTabIndex === 1) {
      return 'Debug';
    }
    return 'Setting';
  }

  private isTaskDetailActive(): boolean {
    return this.selectedTaskId.trim().length > 0 && this.selectedTask !== null;
  }

  @Builder
  private taskTopBar(): void {
    Row() {
      if (this.isTaskDetailActive()) {
        Row() {
          this.taskBarIconButton('chevron_left', THEME_TEXT_PRIMARY, () => {
            this.closeTask();
          })
        }
        .width(TASK_BAR_ACTION_WIDTH)
        .height('100%')
        .justifyContent(FlexAlign.Start)
        .alignItems(VerticalAlign.Center)
      } else {
        Row()
          .width(TASK_BAR_ACTION_WIDTH)
          .height('100%')
      }

      Text(this.isTaskDetailActive() ? '任务详情' : '任务')
        .fontSize(15)
        .fontFamily(FONT_DISPLAY)
        .fontColor(THEME_TEXT_PRIMARY)
        .fontWeight(FontWeight.Bold)
        .layoutWeight(1)
        .textAlign(TextAlign.Center)

      if (this.isTaskDetailActive()) {
        Row({ space: 8 }) {
          this.taskBarIconButton('trash', THEME_ERROR, () => {
            void this.deleteSelectedTask();
          })
        }
        .width(TASK_BAR_ACTION_WIDTH)
        .height('100%')
        .justifyContent(FlexAlign.End)
        .alignItems(VerticalAlign.Center)
      } else {
        Row({ space: 8 }) {
          this.taskBarIconButton('arrow_clockwise', THEME_TEXT_PRIMARY, () => {
            void this.refreshHistory();
          })
          this.taskBarIconButton('clean', THEME_ERROR, () => {
            void this.clearAllHistory();
          })
        }
        .width(TASK_BAR_ACTION_WIDTH)
        .height('100%')
        .justifyContent(FlexAlign.End)
        .alignItems(VerticalAlign.Center)
      }
    }
    .width('100%')
    .height(TASK_BAR_HEIGHT)
    .backgroundColor(Color.White)
    .alignItems(VerticalAlign.Center)
    .padding({ top: 10, bottom: 2 })
  }

  @Builder
  private taskBarIconButton(symbolName: TaskBarSymbolName, color: string, onTap: () => void): void {
    Row() {
      Text() {
        SymbolSpan(this.getTaskBarSymbolResource(symbolName))
          .fontSize(TASK_BAR_ICON_SIZE)
          .fontColor([color])
      }
      .width(TASK_BAR_ICON_SIZE + 6)
      .height(TASK_BAR_ICON_SIZE + 6)
      .textAlign(TextAlign.Center)
      .hitTestBehavior(HitTestMode.None)
    }
    .width(32)
    .height(32)
    .justifyContent(FlexAlign.Center)
    .alignItems(VerticalAlign.Center)
    .backgroundColor('transparent')
    .onClick(onTap)
  }

  private getTaskBarSymbolResource(symbolName: TaskBarSymbolName): Resource {
    if (symbolName === 'arrow_clockwise') {
      return $r('sys.symbol.arrow_clockwise');
    }
    if (symbolName === 'clean') {
      return $r('sys.symbol.clean');
    }
    if (symbolName === 'chevron_left') {
      return $r('sys.symbol.chevron_left');
    }
    return $r('sys.symbol.trash');
  }

  @Builder
  private taskCardItem(task: HistoryTaskMeta): void {
    ListItem() {
      Row() {
        Column({ space: 8 }) {
          Row({ space: 8 }) {
            Text(`${this.resolveTaskStatusLabel(task.status)} / ${this.resolveTaskDuration(task)}`)
              .fontSize(12)
              .fontColor(this.resolveTaskBorderColor(task.status))
              .fontWeight(FontWeight.Bold)
              .layoutWeight(1)

            Image(this.resolveTaskStatusIcon(task.status))
              .width(TASK_CARD_ICON_SIZE)
              .height(TASK_CARD_ICON_SIZE)
              .objectFit(ImageFit.Contain)
          }

          Text(task.taskText)
            .fontSize(15)
            .fontColor(THEME_TEXT_PRIMARY)
            .fontWeight(FontWeight.Bold)
            .maxLines(3)
            .textOverflow({ overflow: TextOverflow.Ellipsis })

          Column({ space: 4 }) {
            Text(`执行结果：${this.resolveTaskResult(task)}`)
              .fontSize(12)
              .fontColor(THEME_TEXT_PRIMARY)
              .maxLines(2)
              .textOverflow({ overflow: TextOverflow.Ellipsis })
          }

          Row() {
            Text(this.formatTime(task.startAtMs))
              .fontSize(11)
              .fontColor(THEME_TEXT_MUTED)
              .layoutWeight(1)

            Text(`steps ${task.stepCount}`)
              .fontSize(11)
              .fontColor(THEME_TEXT_MUTED)
          }
        }
        .width('100%')
        .padding({
          left: TASK_CARD_PADDING_X,
          right: TASK_CARD_PADDING_X,
          top: TASK_CARD_PADDING_TOP,
          bottom: TASK_CARD_PADDING_BOTTOM
        })
        .constraintSize({ minHeight: TASK_CARD_MIN_HEIGHT })
        .borderRadius(16)
        .backgroundColor(this.resolveTaskBackground(task.status))
        .border({ width: 1, color: this.resolveTaskBorderColor(task.status) })
        .shadow({ radius: 12, color: 'rgba(0, 0, 0, 0.08)', offsetX: 0, offsetY: 6 })
      }
      .width('100%')
      .padding({ left: TASK_CARD_SIDE_MARGIN, right: TASK_CARD_SIDE_MARGIN })
    }
    .onClick(() => {
      void this.openTask(task.taskId);
    })
    .swipeAction({
      end: () => {
        this.taskSwipeActions(task);
      }
    })
  }

  @Builder
  private taskSwipeActions(task: HistoryTaskMeta): void {
    Column({ space: 0 }) {
      this.taskDeleteAction(task.taskId, '50%');
      this.taskFavoriteAction(task, '50%');
    }
    .width(TASK_SWIPE_ACTION_WIDTH)
    .height('100%')
    .backgroundColor('transparent')
  }

  @Builder
  private taskDeleteAction(taskId: string, height: string | number = '100%'): void {
    Row() {
      Image($r('app.media.delete'))
        .width(20)
        .height(20)
        .objectFit(ImageFit.Contain)
    }
    .width(TASK_SWIPE_ACTION_WIDTH)
    .height(height)
    .backgroundColor('transparent')
    .justifyContent(FlexAlign.Center)
    .alignItems(VerticalAlign.Center)
    .onClick(() => {
      void this.deleteTaskDirect(taskId);
    })
  }

  @Builder
  private taskFavoriteAction(task: HistoryTaskMeta, height: string | number = '100%'): void {
    Row() {
      Text() {
        SymbolSpan(this.getTaskFavoriteSymbolResource(this.isTaskFavoritedById(task.taskId) ? 'star_fill' : 'star'))
          .fontSize(20)
          .fontColor([this.isTaskFavoritedById(task.taskId) ? TASK_FAVORITE_COLOR : THEME_TEXT_MUTED])
      }
      .width(28)
      .height(28)
      .textAlign(TextAlign.Center)
      .hitTestBehavior(HitTestMode.None)
    }
    .width(TASK_SWIPE_ACTION_WIDTH)
    .height(height)
    .backgroundColor('transparent')
    .justifyContent(FlexAlign.Center)
    .alignItems(VerticalAlign.Center)
    .onClick(() => {
      if (this.isTaskFavoritedById(task.taskId)) {
        void this.setTaskSkillName(task.taskId, '');
      } else {
        this.openFavoriteDialog(task);
      }
    })
  }

  private getTaskFavoriteSymbolResource(symbolName: 'star' | 'star_fill'): Resource {
    if (symbolName === 'star_fill') {
      return $r('sys.symbol.star_fill');
    }
    return $r('sys.symbol.star');
  }

  private isTaskFavorited(task: HistoryTaskMeta): boolean {
    const name = typeof task.skill_name === 'string' ? task.skill_name : '';
    return name.trim().length > 0;
  }

  private isTaskFavoritedById(taskId: string): boolean {
    const id = taskId.trim();
    if (id.length === 0) {
      return false;
    }
    for (let i = 0; i < this.historyTasks.length; i += 1) {
      const task = this.historyTasks[i];
      if (task.taskId === id) {
        return this.isTaskFavorited(task);
      }
    }
    return false;
  }

  private openFavoriteDialog(task: HistoryTaskMeta): void {
    this.favoriteDialogTaskId = task.taskId;
    this.favoriteDialogName = typeof task.skill_name === 'string' ? task.skill_name : '';
    this.favoriteDialogError = '';
    this.favoriteDialogVisible = true;
  }

  private closeFavoriteDialog(): void {
    this.favoriteDialogVisible = false;
    this.favoriteDialogTaskId = '';
    this.favoriteDialogName = '';
    this.favoriteDialogError = '';
  }

  private async confirmFavoriteDialog(): Promise<void> {
    const taskId = this.favoriteDialogTaskId.trim();
    if (taskId.length === 0) {
      this.closeFavoriteDialog();
      return;
    }
    const name = this.favoriteDialogName.trim();
    if (name.length === 0) {
      this.favoriteDialogError = '请填写名称';
      return;
    }
    try {
      await this.setTaskSkillName(taskId, name);
      this.closeFavoriteDialog();
    } catch (e) {
      this.favoriteDialogError = `保存失败: ${String(e)}`;
    }
  }

  private async setTaskSkillName(taskId: string, skillName: string): Promise<void> {
    const id = taskId.trim();
    if (id.length === 0) {
      return;
    }
    const ctx = this.getUiContext();
    const store = TaskHistoryStore.getInstance();
    store.init(ctx);
    await store.updateTaskMeta(id, { skill_name: skillName });
    try {
      await OperationWidgetUpdater.refreshOperationWidgetFavorites(ctx);
    } catch (_e) {
      // ignore widget refresh errors
    }

    this.historyTasks = this.historyTasks.map((t) => {
      if (t.taskId !== id) {
        return t;
      }
      const updated: HistoryTaskMeta = {
        taskId: t.taskId,
        sessionId: t.sessionId,
        taskText: t.taskText,
        cloudEndpoint: t.cloudEndpoint,
        waitAfterActionMs: t.waitAfterActionMs,
        status: t.status,
        startAtMs: t.startAtMs,
        endAtMs: t.endAtMs,
        stepCount: t.stepCount,
        finishContent: t.finishContent,
        error: t.error,
        skill_name: skillName,
      };
      return updated;
    });

    if (this.selectedTaskId === id && this.selectedTask) {
      const current = this.selectedTask;
      const updated: HistoryTaskMeta = {
        taskId: current.taskId,
        sessionId: current.sessionId,
        taskText: current.taskText,
        cloudEndpoint: current.cloudEndpoint,
        waitAfterActionMs: current.waitAfterActionMs,
        status: current.status,
        startAtMs: current.startAtMs,
        endAtMs: current.endAtMs,
        stepCount: current.stepCount,
        finishContent: current.finishContent,
        error: current.error,
        skill_name: skillName,
      };
      this.selectedTask = updated;
    }

    // Force ListItem + swipeAction rebind (ArkUI caches swipe closures).
    this.historyListNonce += 1;
  }

  private resolveTaskIcon(name: 'correct' | 'wrong'): Resource {
    if (name === 'correct') {
      return $r('app.media.correct');
    }
    return $r('app.media.wrong');
  }

  private resolveTaskStatusIcon(status: string): Resource {
    return this.isTaskFinished(status) ? this.resolveTaskIcon('correct') : this.resolveTaskIcon('wrong');
  }

  private resolveTaskBackground(status: string): string {
    return this.isTaskFinished(status) ? TASK_CARD_SUCCESS_BG : TASK_CARD_FAIL_BG;
  }

  private resolveTaskBorderColor(status: string): string {
    return this.isTaskFinished(status) ? TASK_CARD_SUCCESS_BORDER : TASK_CARD_FAIL_BORDER;
  }

  private resolveTaskStatusLabel(status: string): string {
    const normalized = status.trim().toLowerCase();
    if (this.isTaskFinished(normalized)) {
      return '任务成功';
    }
    if (this.isTaskStopped(normalized)) {
      return '手动终止';
    }
    if (this.isTaskFailed(normalized)) {
      return '任务失败';
    }
    return status.trim().length > 0 ? status : '任务失败';
  }

  private resolveTaskResult(task: HistoryTaskMeta): string {
    const normalized = task.status.trim().toLowerCase();
    if (this.isTaskStopped(normalized)) {
      return '用户手动停止了当前任务的执行。';
    }
    if (this.isTaskFinished(normalized)) {
      return task.finishContent.trim().length > 0 ? task.finishContent : '任务成功';
    }
    if (this.isTaskFailed(normalized)) {
      return task.error.trim().length > 0 ? task.error : '任务失败';
    }
    if (task.finishContent.trim().length > 0) {
      return task.finishContent;
    }
    if (task.error.trim().length > 0) {
      return task.error;
    }
    return '任务失败';
  }

  private resolveTaskDuration(task: HistoryTaskMeta): string {
    if (task.startAtMs <= 0 || task.endAtMs <= 0) {
      return '0 s';
    }
    const durationMs = Math.max(0, task.endAtMs - task.startAtMs);
    const totalSeconds = Math.max(0, Math.floor(durationMs / 1000));
    if (totalSeconds < 60) {
      return `${totalSeconds} s`;
    }
    const minutes = Math.floor(totalSeconds / 60);
    const seconds = totalSeconds % 60;
    return `${minutes} min ${seconds} s`;
  }

  private isTaskStopped(status: string): boolean {
    const normalized = status.trim().toLowerCase();
    return normalized === 'stopped' || normalized === 'stop';
  }

  private isTaskFailed(status: string): boolean {
    const normalized = status.trim().toLowerCase();
    return normalized === 'failed' || normalized === 'fail' || normalized === 'error';
  }

  private isTaskFinished(status: string): boolean {
    const normalized = status.trim().toLowerCase();
    return normalized === 'finish' || normalized === 'finished' || normalized === 'success';
  }

  @Builder
  private navSymbol(symbolName: NavSymbolName, active: boolean): void {
    Text() {
      SymbolSpan(this.getNavSymbolResource(symbolName))
        .fontSize(20)
        .fontColor([active ? THEME_ACCENT : THEME_TEXT_SUBTLE])
    }
    .key(`${symbolName}_${active ? 'on' : 'off'}`)
    .width(24)
    .height(24)
    .textAlign(TextAlign.Center)
    .hitTestBehavior(HitTestMode.None)
  }

  private isTabActive(index: number): boolean {
    return this.activeTabIndex === index;
  }

  private getNavSymbolResource(symbolName: NavSymbolName): Resource {
    if (symbolName === 'house_fill') {
      return $r('sys.symbol.house_fill');
    }
    if (symbolName === 'gearshape') {
      return $r('sys.symbol.gearshape');
    }
    return $r('sys.symbol.bolt_filled_on_circle');
  }

  private resolvePanelBorderWidth(): number {
    return this.activeTabIndex === 0 ? 0 : 1;
  }

  private resolvePanelSidePadding(): number {
    return this.activeTabIndex === 0 ? 0 : 16;
  }

  private resolvePanelTopPadding(): number {
    return this.activeTabIndex === 0 ? 0 : 16;
  }

  private resolvePanelBottomPadding(): number {
    // Keep the bottom navigation from covering scrollable contents.
    return NAV_HEIGHT + 16;
  }

  private resolveNavY(): number {
    if (!Number.isFinite(this.viewportHeight) || this.viewportHeight <= 0) {
      return 0;
    }
    return Math.max(0, this.viewportHeight - NAV_HEIGHT);
  }

  @Builder
  private detailTag(text: string, textColor: string, background: string): void {
    Text(text)
      .fontSize(10)
      .fontColor(textColor)
      .padding({ left: 8, right: 8, top: 4, bottom: 4 })
      .backgroundColor(background)
      .borderRadius(10)
  }

  private resolveStatusTagBg(status: string): string {
    return this.isTaskFinished(status) ? DETAIL_STATUS_OK_BG : DETAIL_STATUS_FAIL_BG;
  }

  private resolveStatusTagText(status: string): string {
    return this.isTaskFinished(status) ? DETAIL_STATUS_OK_TEXT : DETAIL_STATUS_FAIL_TEXT;
  }

  private resolveStepStatusLabel(status: string): string {
    const normalized = status.trim().toLowerCase();
    if (normalized.length === 0) {
      return '未知';
    }
    if (normalized === 'success' || normalized === 'ok') {
      return '成功';
    }
    if (normalized === 'failed' || normalized === 'fail' || normalized === 'error') {
      return '失败';
    }
    if (normalized === 'skipped' || normalized === 'skip') {
      return '跳过';
    }
    return status.trim();
  }

  private resolveStepStatusBg(status: string): string {
    const normalized = status.trim().toLowerCase();
    if (normalized.length === 0) {
      return DETAIL_SECTION_BG;
    }
    if (normalized === 'skipped' || normalized === 'skip') {
      return DETAIL_TAG_BG;
    }
    if (this.isTaskFinished(normalized)) {
      return DETAIL_STATUS_OK_BG;
    }
    if (this.isTaskFailed(normalized)) {
      return DETAIL_STATUS_FAIL_BG;
    }
    return DETAIL_SECTION_BG;
  }

  private resolveStepStatusText(status: string): string {
    const normalized = status.trim().toLowerCase();
    if (normalized.length === 0) {
      return THEME_TEXT_MUTED;
    }
    if (normalized === 'skipped' || normalized === 'skip') {
      return DETAIL_TAG_TEXT;
    }
    if (this.isTaskFinished(normalized)) {
      return DETAIL_STATUS_OK_TEXT;
    }
    if (this.isTaskFailed(normalized)) {
      return DETAIL_STATUS_FAIL_TEXT;
    }
    return THEME_TEXT_PRIMARY;
  }

  private resolveActionDisplayName(actionName: string): string {
    const normalized = actionName.trim().toLowerCase();
    if (normalized.length === 0) {
      return '未知操作';
    }
    if (normalized === 'click' || normalized === 'tap') {
      return '点击';
    }
    if (normalized === 'type') {
      return '输入';
    }
    if (normalized === 'double_click') {
      return '双击';
    }
    if (normalized === 'long_click') {
      return '长按';
    }
    if (normalized === 'swipe') {
      return '滑动';
    }
    if (normalized === 'wait') {
      return '等待';
    }
    if (normalized === 'start_app' || normalized === 'open_app' || normalized === 'launch_app') {
      return '打开应用';
    }
    if (normalized === 'press_back') {
      return '返回';
    }
    if (normalized === 'press_enter') {
      return '回车';
    }
    if (normalized === 'call_user') {
      return '请求用户操作';
    }
    if (normalized === 'take_over') {
      return '接管';
    }
    if (normalized === 'take_over') {
      return '任务恢复';
    }
    if (normalized === 'interact') {
      return '交互';
    }
    return actionName.trim();
  }

  private parseActionSnapshot(actionJson: string): AgentActionSnapshot | null {
    const trimmed = actionJson.trim();
    if (trimmed.length === 0) {
      return null;
    }
    try {
      const parsed = JSON.parse(trimmed) as AgentActionSnapshot;
      if (parsed && parsed.name && String(parsed.name).trim().length > 0) {
        return parsed;
      }
      return null;
    } catch (_e) {
      return null;
    }
  }

  private buildStepParamLines(actionJson: string): Array<StepParamLine> {
    const snapshot = this.parseActionSnapshot(actionJson);
    if (!snapshot || !snapshot.parameters) {
      return [];
    }
    const params = snapshot.parameters;
    const lines: Array<StepParamLine> = [];
    if (params.content && params.content.trim().length > 0) {
      lines.push({ label: '内容', value: params.content });
    }
    if (params.point && params.point.length >= 2) {
      lines.push({ label: '位置', value: this.formatNormPoint(params.point) });
    }
    if (params.start_point && params.start_point.length >= 2) {
      lines.push({ label: '起点', value: this.formatNormPoint(params.start_point) });
    }
    if (params.end_point && params.end_point.length >= 2) {
      lines.push({ label: '终点', value: this.formatNormPoint(params.end_point) });
    }
    if (Number.isFinite(params.duration_ms) && (params.duration_ms ?? 0) > 0) {
      lines.push({ label: '时长', value: `${Math.floor(params.duration_ms ?? 0)} ms` });
    }
    if (Number.isFinite(params.seconds) && (params.seconds ?? 0) > 0) {
      lines.push({ label: '等待', value: `${Math.floor(params.seconds ?? 0)} s` });
    }
    if (params.app && params.app.trim().length > 0) {
      lines.push({ label: '应用', value: params.app });
    }
    if (params.options && params.options.length > 0) {
      lines.push({ label: '选项', value: params.options.join(' / ') });
    }
    return lines;
  }

  private resolveStepFooterText(step: HistoryStepMeta): string {
    const paramSummary = this.resolveStepParamSummary(step.actionJson);
    const formatText = step.image.format.trim().length > 0 ? step.image.format : '-';
    const sizeText = this.formatBytes(step.image.bytes);
    if (paramSummary.length > 0) {
      return `关键参数: ${paramSummary} / 格式: ${formatText} / 大小: ${sizeText}`;
    }
    return `格式: ${formatText} / 大小: ${sizeText}`;
  }

  private resolveStepParamSummary(actionJson: string): string {
    const lines = this.buildStepParamLines(actionJson);
    if (lines.length === 0) {
      return '';
    }

    const parts: Array<string> = [];
    for (let i = 0; i < lines.length; i += 1) {
      if (parts.length >= 2) {
        break;
      }
      const line = lines[i];
      const label = line.label.trim();
      const value = this.shortenStepParamValue(label, line.value.trim());
      if (label.length > 0 && value.length > 0) {
        parts.push(`${label}: ${value}`);
      }
    }
    return parts.join(' · ');
  }

  private shortenStepParamValue(label: string, value: string): string {
    if (label === '内容' && value.length > 16) {
      return `${value.slice(0, 16)}…`;
    }
    return value;
  }

  private resolveStepParamPreviewLines(): Array<StepParamLine> {
    if (this.selectedStepParamLines.length <= 3) {
      return this.selectedStepParamLines;
    }
    return this.selectedStepParamLines.slice(0, 3);
  }

  private formatNormPoint(point: Array<number>): string {
    const x = point.length > 0 ? point[0] : 0;
    const y = point.length > 1 ? point[1] : 0;
    const xPct = this.formatPercent(x);
    const yPct = this.formatPercent(y);
    return `${xPct}, ${yPct}`;
  }

  private formatPercent(value: number): string {
    if (!Number.isFinite(value)) {
      return '-';
    }
    if (value >= 0 && value <= 1) {
      return `${Math.round(value * 100)}%`;
    }
    return `${Math.round(value)}px`;
  }

  private resolveStepResultSummary(step: HistoryStepMeta): string {
    const normalized = step.resultStatus.trim().toLowerCase();
    const output = step.output.trim();
    const error = step.error.trim();
    if (normalized === 'failed' || normalized === 'fail' || normalized === 'error') {
      if (error.length > 0) {
        return error;
      }
      return output.length > 0 ? output : '执行失败';
    }
    if (normalized === 'skipped' || normalized === 'skip') {
      return output.length > 0 ? output : '已跳过';
    }
    if (normalized.length === 0) {
      return output.length > 0 ? output : '-';
    }
    return output.length > 0 ? output : '执行成功';
  }

  private resolveStepResultBg(step: HistoryStepMeta): string {
    const normalized = step.resultStatus.trim().toLowerCase();
    if (normalized === 'failed' || normalized === 'fail' || normalized === 'error') {
      return DETAIL_STATUS_FAIL_BG;
    }
    if (normalized === 'skipped' || normalized === 'skip') {
      return DETAIL_SECTION_BG;
    }
    if (normalized.length === 0) {
      return DETAIL_SECTION_BG;
    }
    return DETAIL_STATUS_OK_BG;
  }

  private resolveStepResultBorder(step: HistoryStepMeta): string {
    const normalized = step.resultStatus.trim().toLowerCase();
    if (normalized === 'failed' || normalized === 'fail' || normalized === 'error') {
      return DETAIL_STATUS_FAIL_TEXT;
    }
    if (normalized === 'skipped' || normalized === 'skip') {
      return DETAIL_SECTION_BORDER;
    }
    if (normalized.length === 0) {
      return DETAIL_SECTION_BORDER;
    }
    return DETAIL_STATUS_OK_TEXT;
  }

  private resolveStepResultTextColor(step: HistoryStepMeta): string {
    const normalized = step.resultStatus.trim().toLowerCase();
    if (normalized === 'failed' || normalized === 'fail' || normalized === 'error') {
      return THEME_ERROR;
    }
    return THEME_TEXT_PRIMARY;
  }

  private resolveStepSwiperIndex(): number {
    if (this.selectedSteps.length === 0) {
      return 0;
    }
    const clamped = Math.max(0, Math.min(this.selectedStepPosition, this.selectedSteps.length - 1));
    return clamped;
  }

  private onStepChange(index: number): void {
    this.selectStepAtPosition(index);
  }

  private selectStepAtPosition(position: number): void {
    if (this.selectedSteps.length === 0) {
      this.selectedStepPosition = 0;
      this.selectedStepIndex = 0;
      this.selectedStep = null;
      this.selectedStepPixelMap = null;
      this.selectedStepParamLines = [];
      this.showStepMore = false;
      return;
    }

    const clamped = Math.max(0, Math.min(position, this.selectedSteps.length - 1));
    this.selectedStepPosition = clamped;
    const step = this.selectedSteps[clamped];
    if (!step) {
      this.selectedStepIndex = 0;
      this.selectedStep = null;
      this.selectedStepPixelMap = null;
      this.selectedStepParamLines = [];
      this.showStepMore = false;
      return;
    }

    this.selectedStepIndex = step.stepIndex;
    this.selectedStep = step;
    this.selectedStepPixelMap = null;
    this.selectedStepParamLines = this.buildStepParamLines(step.actionJson);
    this.showStepMore = false;
    this.ensureStepPixelMapLoaded(step.taskId, step.stepIndex, clamped);
    void this.loadStepPixelMap(step.taskId, step.stepIndex);
  }

  private async loadStepPixelMap(taskId: string, stepIndex: number): Promise<void> {
    const requestSeq = this.stepPixelMapRequestSeq + 1;
    this.stepPixelMapRequestSeq = requestSeq;
    try {
      const ctx = this.getUiContext();
      const store = TaskHistoryStore.getInstance();
      store.init(ctx);
      const pixelMap = await store.getStepPixelMap(taskId, stepIndex);
      if (requestSeq !== this.stepPixelMapRequestSeq) {
        return;
      }
      if (this.selectedTaskId !== taskId || this.selectedStepIndex !== stepIndex) {
        return;
      }
      this.selectedStepPixelMap = pixelMap;
    } catch (_e) {
      // ignore
    }
  }

  private isStepPositionActive(position: number): boolean {
    return position === this.selectedStepPosition;
  }

  @Builder
  private stepParamSection(actionJson: string): void {
    if (this.resolveStepParamLines(actionJson).length > 0) {
      Column({ space: 6 }) {
        Text('关键参数')
          .fontSize(DETAIL_TITLE_SIZE)
          .fontColor(THEME_TEXT_PRIMARY)
          .fontWeight(FontWeight.Medium)

        ForEach(this.resolveStepParamLines(actionJson), (line: StepParamLine) => {
          this.detailRow(line.label, line.value)
        }, (line: StepParamLine) => line.label)
      }
      .padding(12)
      .borderRadius(DETAIL_SECTION_RADIUS)
      .backgroundColor(DETAIL_SECTION_BG)
      .border({ width: 1, color: DETAIL_SECTION_BORDER })
    }
  }

  private resolveStepParamLines(actionJson: string): Array<StepParamLine> {
    return this.buildStepParamLines(actionJson);
  }

  private resolveStepPixelMap(position: number): image.PixelMap | null {
    if (position < 0 || position >= this.stepPixelMaps.length) {
      return null;
    }
    return this.stepPixelMaps[position] ?? null;
  }

  private resolveStepPixelMapLoading(position: number): boolean {
    if (position < 0 || position >= this.stepPixelMapLoading.length) {
      return false;
    }
    return this.stepPixelMapLoading[position] ?? false;
  }

  private resetStepScreenshotCache(length: number): void {
    this.stepImageCacheSeq += 1;
    const pixelMaps: Array<image.PixelMap | null> = [];
    const loading: Array<boolean> = [];
    for (let i = 0; i < length; i += 1) {
      pixelMaps.push(null);
      loading.push(false);
    }
    this.stepPixelMaps = pixelMaps;
    this.stepPixelMapLoading = loading;
  }

  private ensureStepPixelMapLoaded(taskId: string, stepIndex: number, position: number): void {
    if (position < 0 || position >= this.selectedSteps.length) {
      return;
    }
    if (position < this.stepPixelMaps.length && this.stepPixelMaps[position]) {
      return;
    }
    if (position < this.stepPixelMapLoading.length && this.stepPixelMapLoading[position]) {
      return;
    }

    const nextLoading = this.stepPixelMapLoading.slice();
    while (nextLoading.length < this.selectedSteps.length) {
      nextLoading.push(false);
    }
    nextLoading[position] = true;
    this.stepPixelMapLoading = nextLoading;

    const requestSeq = this.stepImageCacheSeq;
    void this.loadStepPixelMapForPosition(requestSeq, taskId, stepIndex, position);
  }

  private async loadStepPixelMapForPosition(
    requestSeq: number,
    taskId: string,
    stepIndex: number,
    position: number
  ): Promise<void> {
    try {
      const ctx = this.getUiContext();
      const store = TaskHistoryStore.getInstance();
      store.init(ctx);
      const pixelMap = await store.getStepPixelMap(taskId, stepIndex);
      if (requestSeq !== this.stepImageCacheSeq) {
        return;
      }
      if (this.selectedTaskId !== taskId) {
        return;
      }
      if (position < 0 || position >= this.selectedSteps.length) {
        return;
      }
      const step = this.selectedSteps[position];
      if (!step || step.stepIndex !== stepIndex) {
        return;
      }

      const next = this.stepPixelMaps.slice();
      while (next.length < this.selectedSteps.length) {
        next.push(null);
      }
      next[position] = pixelMap;
      this.stepPixelMaps = next;
    } catch (_e) {
      // ignore
    } finally {
      if (requestSeq !== this.stepImageCacheSeq) {
        return;
      }
      const nextLoading = this.stepPixelMapLoading.slice();
      if (position >= 0 && position < nextLoading.length) {
        nextLoading[position] = false;
        this.stepPixelMapLoading = nextLoading;
      }
    }
  }

  private findStepPosition(stepIndex: number): number {
    if (this.selectedSteps.length === 0) {
      return 0;
    }
    const position = this.selectedSteps.findIndex((step) => step.stepIndex === stepIndex);
    return position >= 0 ? position : 0;
  }

  private formatActionJson(actionJson: string): string {
    const trimmed = actionJson.trim();
    if (trimmed.length === 0) {
      return '-';
    }
    try {
      const parsed: JsonLike = JSON.parse(trimmed) as JsonLike;
      return JSON.stringify(parsed, null, 2);
    } catch (e) {
      return trimmed;
    }
  }

  private formatBytes(bytes: number): string {
    if (!Number.isFinite(bytes) || bytes <= 0) {
      return '0 B';
    }
    if (bytes < 1024) {
      return `${bytes} B`;
    }
    const kb = bytes / 1024;
    if (kb < 1024) {
      return `${kb.toFixed(1)} KB`;
    }
    const mb = kb / 1024;
    return `${mb.toFixed(1)} MB`;
  }

  build() {
    Stack() {
      Column({ space: 0 }) {
        Column({ space: 12 }) {
          if (this.activeTabIndex !== 0) {
            this.pageHeader();
          }
          this.activeTabContent();
        }
        .width('100%')
        .layoutWeight(1)
        .padding({
          left: this.resolvePanelSidePadding(),
          right: this.resolvePanelSidePadding(),
          top: this.resolvePanelTopPadding(),
          bottom: this.resolvePanelBottomPadding()
        })
        .backgroundColor(THEME_PANEL)
        .borderRadius(26)
        .border({ width: this.resolvePanelBorderWidth(), color: THEME_BORDER })
      }
      .width('100%')
      .height('100%')

      this.bottomNav()

      if (this.favoriteDialogVisible) {
        this.favoriteNameDialog();
      }
    }
    .width('100%')
    .height('100%')
    .padding({ left: 0, right: 0, top: SAFE_TOP_PADDING, bottom: 0 })
    .backgroundColor(THEME_BG_START)
    .onAreaChange((_oldValue, newValue) => {
      this.viewportHeight = Number(newValue.height);
    })
  }

  @Builder
  private favoriteNameDialog(): void {
    Stack() {
      Row()
        .width('100%')
        .height('100%')
        .backgroundColor('rgba(0, 0, 0, 0.35)')

      Column({ space: 10 }) {
        Text('收藏为技能')
          .fontSize(14)
          .fontFamily(FONT_DISPLAY)
          .fontColor(THEME_TEXT_PRIMARY)
          .fontWeight(FontWeight.Medium)

        TextInput({ placeholder: '请输入名称', text: this.favoriteDialogName })
          .height(36)
          .fontSize(12)
          .fontColor(THEME_TEXT_PRIMARY)
          .backgroundColor(THEME_CARD)
          .borderRadius(10)
          .border({ width: 1, color: THEME_BORDER })
          .padding({ left: 10, right: 10 })
          .onChange((value: string) => {
            this.favoriteDialogName = value;
            if (this.favoriteDialogError.length > 0) {
              this.favoriteDialogError = '';
            }
          })

        if (this.favoriteDialogError.length > 0) {
          Text(this.favoriteDialogError)
            .fontSize(11)
            .fontColor(THEME_ERROR)
        }

        Row({ space: 10 }) {
          Button('取消')
            .layoutWeight(1)
            .height(36)
            .fontSize(12)
            .fontColor(THEME_TEXT_PRIMARY)
            .backgroundColor('rgba(0, 0, 0, 0.06)')
            .borderRadius(10)
            .onClick(() => {
              this.closeFavoriteDialog();
            })

          Button('保存')
            .layoutWeight(1)
            .height(36)
            .fontSize(12)
            .fontColor(Color.White)
            .backgroundColor(THEME_ACCENT)
            .borderRadius(10)
            .onClick(() => {
              void this.confirmFavoriteDialog();
            })
        }
        .width('100%')
      }
      .width('86%')
      .padding(14)
      .backgroundColor(THEME_PANEL)
      .borderRadius(16)
      .border({ width: 1, color: THEME_BORDER })
    }
    .width('100%')
    .height('100%')
  }

  private getUiContext(): common.UIAbilityContext {
    return getContext(this) as common.UIAbilityContext;
  }

  private ensureWriteImageVideoPermission(): Promise<void> {
    const ctx = this.getUiContext();
    const atManager = abilityAccessCtrl.createAtManager();
    const permission: Permissions = 'ohos.permission.WRITE_IMAGEVIDEO';
    const tokenId = ctx.applicationInfo.accessTokenId;
    return atManager.checkAccessToken(tokenId, permission)
      .then((status: abilityAccessCtrl.GrantStatus) => {
        if (status === abilityAccessCtrl.GrantStatus.PERMISSION_GRANTED) {
          return;
        }
        return atManager.requestPermissionsFromUser(ctx, [permission])
          .then((result) => {
            const granted = result.authResults.length > 0 &&
              result.authResults.every((item: number) => item === abilityAccessCtrl.GrantStatus.PERMISSION_GRANTED);
            if (!granted) {
              console.warn('WRITE_IMAGEVIDEO permission denied');
            }
          });
      })
      .catch((error: BusinessError) => {
        console.error(`WRITE_IMAGEVIDEO permission request failed, code: ${error.code}, message: ${error.message}`);
      });
  }

  private ensureMicrophonePermission(): Promise<void> {
    const ctx = this.getUiContext();
    const atManager = abilityAccessCtrl.createAtManager();
    const permission: Permissions = 'ohos.permission.MICROPHONE';
    const tokenId = ctx.applicationInfo.accessTokenId;
    return atManager.checkAccessToken(tokenId, permission)
      .then((status: abilityAccessCtrl.GrantStatus) => {
        if (status === abilityAccessCtrl.GrantStatus.PERMISSION_GRANTED) {
          return;
        }
        return atManager.requestPermissionsFromUser(ctx, [permission])
          .then((result) => {
            const granted = result.authResults.length > 0 &&
              result.authResults.every((item: number) => item === abilityAccessCtrl.GrantStatus.PERMISSION_GRANTED);
            if (!granted) {
              console.warn('MICROPHONE permission denied');
            }
          });
      })
      .catch((error: BusinessError) => {
        console.error(`MICROPHONE permission request failed, code: ${error.code}, message: ${error.message}`);
      });
  }

  private ensureLocationPermission(): Promise<void> {
    const ctx = this.getUiContext();
    const atManager = abilityAccessCtrl.createAtManager();
    const permission: Permissions = 'ohos.permission.APPROXIMATELY_LOCATION';
    const tokenId = ctx.applicationInfo.accessTokenId;
    return atManager.checkAccessToken(tokenId, permission)
      .then((status: abilityAccessCtrl.GrantStatus) => {
        if (status === abilityAccessCtrl.GrantStatus.PERMISSION_GRANTED) {
          return;
        }
        return atManager.requestPermissionsFromUser(ctx, [permission])
          .then((result) => {
            const granted = result.authResults.length > 0 &&
              result.authResults.every((item: number) => item === abilityAccessCtrl.GrantStatus.PERMISSION_GRANTED);
            if (!granted) {
              console.warn('APPROXIMATELY_LOCATION permission denied');
            }
          });
      })
      .catch((error: BusinessError) => {
        console.error(`APPROXIMATELY_LOCATION permission request failed, code: ${error.code}, message: ${error.message}`);
      });
  }

  private async loadSettings(): Promise<void> {
    this.settingsInfo = '';
    try {
      const ctx = this.getUiContext();
      const settings = await AgentSettings.load(ctx);
      this.cloudEndpoint = settings.cloudEndpoint;
      this.a2aBaseUrl = settings.a2aBaseUrl;
      this.mcpWsUrl = settings.mcpWsUrl;
      this.deviceId = settings.deviceId;
      this.contextId = settings.contextId;
      this.waitMsText = String(settings.waitMs);
      this.liveViewEnabled = settings.liveViewEnabled;
      this.historyEnabled = settings.historyEnabled;
      this.historyMaxTasksText = String(settings.historyMaxTasks);
      TaskHistoryStore.getInstance().init(ctx);
      TaskHistoryStore.getInstance().configure(settings.historyEnabled, settings.historyMaxTasks);
    } catch (e) {
      this.settingsInfo = `加载配置失败: ${String(e)}`;
    }
  }

  private async saveSettings(): Promise<void> {
    this.settingsInfo = '';
    try {
      const ctx = this.getUiContext();
      const waitMs = this.parseNonNegativeInt(this.waitMsText, 500);
      const maxTasks = this.parseNonNegativeInt(this.historyMaxTasksText, 50);
      const data: AgentSettingsData = {
        cloudEndpoint: this.cloudEndpoint.trim(),
        a2aBaseUrl: this.a2aBaseUrl.trim(),
        mcpWsUrl: this.mcpWsUrl.trim(),
        deviceId: this.deviceId.trim(),
        contextId: this.contextId.trim(),
        waitMs,
        liveViewEnabled: this.liveViewEnabled,
        historyEnabled: this.historyEnabled,
        historyMaxTasks: maxTasks,
      };
      await AgentSettings.save(ctx, data);
      TaskHistoryStore.getInstance().init(ctx);
      TaskHistoryStore.getInstance().configure(data.historyEnabled, data.historyMaxTasks);
      this.settingsInfo = '配置已保存';
      await this.refreshHistory();
    } catch (e) {
      this.settingsInfo = `保存失败: ${String(e)}`;
    }
  }

  private async loadWallpaper(): Promise<void> {
    this.wallpaperInfo = '正在获取壁纸...';
    this.wallpaperPixelMap = null;
    try {
      const pixelMap = await WallpaperState.getCurrentWallpaper();
      if (pixelMap) {
        this.wallpaperPixelMap = pixelMap;
        this.wallpaperInfo = '壁纸获取成功';
      } else {
        this.wallpaperInfo = '未获取到壁纸';
      }
    } catch (e) {
      this.wallpaperInfo = `壁纸获取失败: ${String(e)}`;
      this.wallpaperPixelMap = null;
    }
  }

  private async loadForegroundApp(): Promise<void> {
    this.foregroundAppInfo = '正在获取前台应用...';
    try {
      const bundleName = await ForegroundAppState.getForegroundAppBundleName();
      if (bundleName.length > 0) {
        this.foregroundAppInfo = `前台应用: ${bundleName}`;
      } else {
        this.foregroundAppInfo = '未检测到前台应用';
      }
    } catch (e) {
      this.foregroundAppInfo = `获取前台应用失败: ${String(e)}`;
    }
  }

  private async loadLocation(): Promise<void> {
    this.locationInfo = '正在获取定位...';
    try {
      const location = await LocationState.getCurrentLocation();
      if (location) {
        const lat = location.latitude.toFixed(10);
        const lon = location.longitude.toFixed(10);
        const acc = Math.round(location.accuracy);
        let info = `定位: ${lat}, ${lon} (±${acc}m)`;
        const address = await LocationState.getAddressFromLocation(location.latitude, location.longitude);
        if (address.length > 0) {
          info = `${info}\n地址: ${address}`;
        }
        this.locationInfo = info;
      } else {
        this.locationInfo = '未获取到定位';
      }
    } catch (e) {
      this.locationInfo = `定位失败: ${String(e)}`;
    }
  }

  private async openAgentSpaceViewer(): Promise<void> {
    try {
      const ctx = this.getUiContext();
      const want: Want = {
        bundleName: 'com.huawei.aios.agent.operation',
        abilityName: 'AgentSpaceViewerAbility',
      };
      await ctx.startAbility(want);
    } catch (e) {
      console.error(`打开 AgentSpaceViewer 失败: ${String(e)}`);
    }
  }

  private async ensureAgentServiceConnected(): Promise<void> {
    const ctx = this.getUiContext();
    await this.agentServiceClient.connect(ctx);
  }

  private async pullDebugEvents(): Promise<void> {
    this.debugEventInfo = '';
    try {
      await this.ensureAgentServiceConnected();
      const payload = await this.agentServiceClient.pullEvents(this.debugEventSequence);
      if (payload.length === 0) {
        if (this.debugEvents.length === 0) {
          this.debugEventInfo = '暂无事件';
        }
        return;
      }
      const batch = this.parseEventBatch(payload);
      if (!batch) {
        this.debugEventInfo = '事件解析失败';
        return;
      }
      this.debugEventSequence = batch.sequence;
      this.debugEvents = this.mergeDebugEvents(batch.events);
    } catch (e) {
      this.debugEventInfo = `拉取事件失败: ${String(e)}`;
    }
  }

  private clearDebugEvents(): void {
    this.debugEvents = [];
    this.debugEventSequence = 0;
    this.debugEventInfo = '已清空事件';
  }

  private parseEventBatch(payload: string): TaskEventBatch | null {
    try {
      const parsed = JSON.parse(payload) as TaskEventBatch;
      if (!parsed || !Array.isArray(parsed.events)) {
        return null;
      }
      return parsed;
    } catch (_e) {
      return null;
    }
  }

  private mergeDebugEvents(incoming: Array<TaskEvent>): Array<TaskEvent> {
    const combined = this.debugEvents.concat(incoming);
    const limit = 50;
    if (combined.length <= limit) {
      return combined;
    }
    return combined.slice(combined.length - limit);
  }

  private formatEventLine(event: TaskEvent): string {
    const msg = event.message.length > 0 ? event.message : '-';
    return `#${event.seq} ${event.type}: ${msg}`;
  }

  private formatEventPayload(payload: string): string {
    const maxLen = 180;
    if (payload.length <= maxLen) {
      return payload;
    }
    return `${payload.slice(0, maxLen)}...`;
  }

  private async simulateToolCall(): Promise<void> {
    const name = this.debugToolName.trim();
    if (name.length === 0) {
      this.debugToolResult = '请输入工具名';
      return;
    }
    const rawArgs = this.debugToolArgs.trim();
    const argsJson = rawArgs.length > 0 ? rawArgs : '{}';
    try {
      await this.ensureAgentServiceConnected();
      const payload = await this.agentServiceClient.debugSimulateToolCall(name, argsJson);
      this.debugToolResult = payload.length > 0 ? payload : 'ok';
    } catch (e) {
      this.debugToolResult = `执行失败: ${String(e)}`;
    }
  }

  private formatDebugToolResult(payload: string): string {
    const maxLen = 260;
    if (payload.length <= maxLen) {
      return payload;
    }
    return `${payload.slice(0, maxLen)}...`;
  }

  private async connectMcp(): Promise<void> {
    this.mcpStatusInfo = '正在连接...';
    try {
      await this.ensureAgentServiceConnected();
      const payload = await this.agentServiceClient.mcpConnect();
      this.mcpStatusInfo = this.formatMcpStatus(payload);
    } catch (e) {
      this.mcpStatusInfo = `连接失败: ${String(e)}`;
    }
  }

  private async disconnectMcp(): Promise<void> {
    this.mcpStatusInfo = '正在断开...';
    try {
      await this.ensureAgentServiceConnected();
      const payload = await this.agentServiceClient.mcpDisconnect();
      this.mcpStatusInfo = this.formatMcpStatus(payload);
    } catch (e) {
      this.mcpStatusInfo = `断开失败: ${String(e)}`;
    }
  }

  private async refreshMcpStatus(): Promise<void> {
    this.mcpStatusInfo = '正在读取状态...';
    try {
      await this.ensureAgentServiceConnected();
      const payload = await this.agentServiceClient.mcpStatus();
      this.mcpStatusInfo = this.formatMcpStatus(payload);
    } catch (e) {
      this.mcpStatusInfo = `状态读取失败: ${String(e)}`;
    }
  }

  private formatMcpStatus(payload: string): string {
    const trimmed = payload.trim();
    if (trimmed.length === 0) {
      return '无状态';
    }
    if (!trimmed.startsWith('{')) {
      return trimmed;
    }
    try {
      const parsed = JSON.parse(trimmed) as McpStatusSnapshot;
      const parts: Array<string> = [];
      const state = parsed.state ?? '';
      const url = parsed.url ?? '';
      const error = parsed.error ?? '';
      if (state.length > 0) {
        parts.push(`state=${state}`);
      }
      if (url.length > 0) {
        parts.push(`url=${url}`);
      }
      if (error.length > 0) {
        parts.push(`error=${error}`);
      }
      if (parts.length > 0) {
        return parts.join(' / ');
      }
      return trimmed;
    } catch (_e) {
      return trimmed;
    }
  }

  private async refreshWidgetForms(): Promise<void> {
    this.widgetFormInfo = '正在读取...';
    try {
      const records = await WidgetFormStore.list(this.getUiContext());
      this.widgetForms = records;
      this.widgetFormInfo = `formId 数量: ${records.length}`;
    } catch (e) {
      this.widgetForms = [];
      this.widgetFormInfo = `读取失败: ${String(e)}`;
    }
  }

  private async clearWidgetForms(): Promise<void> {
    this.widgetFormInfo = '';
    try {
      await WidgetFormStore.clear(this.getUiContext());
      this.widgetForms = [];
      this.widgetFormInfo = '已清空';
    } catch (e) {
      this.widgetFormInfo = `清空失败: ${String(e)}`;
    }
  }

  private async replayHistoryTask(taskId: string): Promise<void> {
    const id = taskId.trim();
    if (id.length === 0) {
      return;
    }
    this.historyError = '';
    this.historyNotice = '正在启动回放...';
    try {
      await this.ensureAgentServiceConnected();
      const payload = await this.agentServiceClient.replayTask(id);
      if (payload.startsWith('ERR_')) {
        this.historyNotice = '';
        this.historyError = `回放失败: ${payload}`;
        return;
      }
      this.historyNotice = '回放已启动';
    } catch (e) {
      this.historyNotice = '';
      this.historyError = `回放失败: ${String(e)}`;
    }
  }

  private async refreshWidgetFavorites(): Promise<void> {
    this.widgetFavoritesInfo = '正在刷新卡片内容...';
    try {
      await OperationWidgetUpdater.refreshOperationWidgetFavorites(this.getUiContext());
      this.widgetFavoritesInfo = '已触发桌面卡片刷新';
    } catch (e) {
      this.widgetFavoritesInfo = `刷新失败: ${String(e)}`;
    }
  }

  private formatWidgetFormLine(record: WidgetFormRecord): string {
    const dim = record.dimension.trim().length > 0 ? record.dimension.trim() : '-';
    const ts = this.formatWidgetFormTime(record.updatedAtMs);
    const name = record.widgetName.trim().length > 0 ? record.widgetName.trim() : '-';
    return `${record.formId} / ${name} / ${dim} / ${ts}`;
  }

  private formatWidgetFormTime(tsMs: number): string {
    if (!Number.isFinite(tsMs) || tsMs <= 0) {
      return '-';
    }
    const date = new Date(tsMs);
    const hh = String(date.getHours()).padStart(2, '0');
    const mm = String(date.getMinutes()).padStart(2, '0');
    const ss = String(date.getSeconds()).padStart(2, '0');
    return `${hh}:${mm}:${ss}`;
  }

  private async refreshMcpLogs(): Promise<void> {
    this.mcpLogInfo = '';
    try {
      await this.ensureAgentServiceConnected();
      const payload = await this.agentServiceClient.mcpLogs();
      const lines = this.parseMcpLogLines(payload);
      this.mcpLogLines = lines;
      if (lines.length === 0) {
        this.mcpLogInfo = '暂无日志';
      }
    } catch (e) {
      this.mcpLogInfo = `读取日志失败: ${String(e)}`;
    }
  }

  private async clearMcpLogs(): Promise<void> {
    this.mcpLogInfo = '';
    try {
      await this.ensureAgentServiceConnected();
      await this.agentServiceClient.mcpClearLogs();
      this.mcpLogLines = [];
      this.mcpLogInfo = '日志已清空';
    } catch (e) {
      this.mcpLogInfo = `清空日志失败: ${String(e)}`;
    }
  }

  private parseMcpLogLines(payload: string): Array<string> {
    const trimmed = payload.trim();
    if (trimmed.length === 0) {
      return [];
    }
    if (!trimmed.startsWith('[')) {
      return [trimmed];
    }
    try {
      const parsed = JSON.parse(trimmed) as Array<string>;
      if (!Array.isArray(parsed)) {
        return [trimmed];
      }
      const lines: Array<string> = [];
      for (let i = 0; i < parsed.length; i += 1) {
        const value = parsed[i];
        if (typeof value === 'string') {
          lines.push(value);
        }
      }
      return lines;
    } catch (_e) {
      return [trimmed];
    }
  }

  private async a2aSendBlocking(): Promise<void> {
    this.a2aResultInfo = '';
    this.a2aResult = '';
    const prompt = this.a2aPrompt.trim();
    if (prompt.length === 0) {
      this.a2aResultInfo = '请输入任务指令';
      return;
    }
    try {
      await this.ensureAgentServiceConnected();
      const payload = await this.agentServiceClient.a2aSendBlocking(prompt);
      this.applyA2AResponse(payload);
    } catch (e) {
      this.a2aResultInfo = `发送失败: ${String(e)}`;
    }
  }

  private async a2aResumeBlocking(): Promise<void> {
    this.a2aResultInfo = '';
    this.a2aResult = '';
    const taskId = this.a2aTaskId.trim();
    if (taskId.length === 0) {
      this.a2aResultInfo = '请输入 taskId';
      return;
    }
    const reply = this.a2aReply.trim();
    if (reply.length === 0) {
      this.a2aResultInfo = '请输入回复内容';
      return;
    }
    try {
      await this.ensureAgentServiceConnected();
      const payload = await this.agentServiceClient.a2aResumeBlocking(taskId, reply);
      this.applyA2AResponse(payload);
    } catch (e) {
      this.a2aResultInfo = `续跑失败: ${String(e)}`;
    }
  }

  private async startA2AStream(): Promise<void> {
    this.a2aStreamInfo = '';
    const prompt = this.a2aStreamPrompt.trim();
    if (prompt.length === 0) {
      this.a2aStreamInfo = '请输入 stream prompt';
      return;
    }
    const contextId = this.a2aStreamContextId.trim();
    try {
      await this.ensureAgentServiceConnected();
      const payload = await this.agentServiceClient.a2aStartStream(prompt, contextId);
      this.a2aStreamInfo = payload.length > 0 ? payload : 'stream started';
    } catch (e) {
      this.a2aStreamInfo = `启动失败: ${String(e)}`;
    }
  }

  private async stopA2AStream(): Promise<void> {
    this.a2aStreamInfo = '';
    try {
      await this.ensureAgentServiceConnected();
      await this.agentServiceClient.a2aStopStream();
      this.a2aStreamInfo = 'stream stopped';
    } catch (e) {
      this.a2aStreamInfo = `停止失败: ${String(e)}`;
    }
  }

  private async pullA2AStreamEvents(): Promise<void> {
    this.a2aStreamInfo = '';
    try {
      await this.ensureAgentServiceConnected();
      const payload = await this.agentServiceClient.a2aPullEvents(this.a2aStreamSequence);
      if (payload.length === 0) {
        if (this.a2aStreamEvents.length === 0) {
          this.a2aStreamInfo = '暂无A2A事件';
        }
        return;
      }
      const batch = this.parseEventBatch(payload);
      if (!batch) {
        this.a2aStreamInfo = '事件解析失败';
        return;
      }
      this.a2aStreamSequence = batch.sequence;
      this.a2aStreamEvents = this.mergeA2AEvents(batch.events);
    } catch (e) {
      this.a2aStreamInfo = `拉取失败: ${String(e)}`;
    }
  }

  private clearA2AStreamEvents(): void {
    this.a2aStreamEvents = [];
    this.a2aStreamSequence = 0;
    this.a2aStreamInfo = '已清空A2A事件';
  }

  private mergeA2AEvents(incoming: Array<TaskEvent>): Array<TaskEvent> {
    const combined = this.a2aStreamEvents.concat(incoming);
    const limit = 80;
    if (combined.length <= limit) {
      return combined;
    }
    return combined.slice(combined.length - limit);
  }

  private applyA2AResponse(payload: string): void {
    this.a2aResult = payload;
    const task = this.parseA2ATask(payload);
    if (!task) {
      this.a2aResultInfo = '返回解析失败';
      return;
    }
    const taskId = task.id ?? '';
    const state = task.status?.state ?? '';
    const msg = this.extractA2AMessage(task);
    const parts: Array<string> = [];
    if (taskId.length > 0) {
      parts.push(`taskId=${taskId}`);
    }
    if (state.length > 0) {
      parts.push(`state=${state}`);
    }
    if (msg.length > 0) {
      parts.push(`message=${msg}`);
    }
    if (parts.length > 0) {
      this.a2aResultInfo = parts.join(' / ');
    }
    if (this.a2aTaskId.trim().length === 0 && taskId.length > 0) {
      this.a2aTaskId = taskId;
    }
  }

  private parseA2ATask(payload: string): A2ATaskSnapshot | null {
    const trimmed = payload.trim();
    if (trimmed.length === 0) {
      return null;
    }
    try {
      const parsed = JSON.parse(trimmed) as A2AResponseSnapshot;
      if (!parsed || typeof parsed !== 'object') {
        return null;
      }
      if (parsed.task && typeof parsed.task === 'object') {
        return parsed.task as A2ATaskSnapshot;
      }
      const taskCandidate = parsed as A2ATaskSnapshot;
      if (taskCandidate.id || taskCandidate.status) {
        return taskCandidate;
      }
      return null;
    } catch (_e) {
      return null;
    }
  }

  private extractA2AMessage(task: A2ATaskSnapshot): string {
    const parts = task.status?.message?.parts;
    if (!parts || parts.length === 0) {
      return '';
    }
    const first = parts[0];
    if (first && typeof first.text === 'string') {
      return this.truncateText(first.text.trim(), 80);
    }
    return '';
  }

  private formatA2AResult(payload: string): string {
    const maxLen = 260;
    if (payload.length <= maxLen) {
      return payload;
    }
    return `${payload.slice(0, maxLen)}...`;
  }

  private truncateText(text: string, maxLen: number): string {
    if (text.length <= maxLen) {
      return text;
    }
    return `${text.slice(0, maxLen)}...`;
  }

  private async initAgentSpace(): Promise<void> {
    this.agentSpaceInfo = '正在初始化 AgentSpace...';
    try {
      await this.ensureAgentServiceConnected();
      const runtimeJson = await this.agentServiceClient.ensureVts();
      this.agentSpaceInfo = runtimeJson.length > 0 ? `AgentSpace 已初始化: ${runtimeJson}` : 'AgentSpace 已初始化';
    } catch (e) {
      this.agentSpaceInfo = `初始化 AgentSpace 失败: ${String(e)}`;
    }
  }

  private async openAgentSpaceApp(): Promise<void> {
    const name = this.agentSpaceAppName.trim();
    if (name.length === 0) {
      this.agentSpaceInfo = '请输入应用名称';
      return;
    }
    this.agentSpaceInfo = `正在打开应用: ${name}`;
    try {
      await this.ensureAgentServiceConnected();
      await this.agentServiceClient.launchAppByName(name);
      this.agentSpaceInfo = `已下发打开应用: ${name}`;
    } catch (e) {
      this.agentSpaceInfo = `打开应用失败: ${String(e)}`;
    }
  }

  private async debugGetA11yTreeList(): Promise<void> {
    this.a11yTreeListInfo = '正在获取 UI 树列表...';
    try {
      await this.ensureAgentServiceConnected();
      const payload = await this.agentServiceClient.getA11yTreeList(0);
      this.a11yTreeListInfo = payload.length > 0 ? payload : 'UI 树列表为空';
    } catch (e) {
      this.a11yTreeListInfo = `获取 UI 树列表失败: ${String(e)}`;
    }
  }

  private async showGlobalFloatWindow(): Promise<void> {
    try {
      const ctx = this.getUiContext();
      const want: Want = {
        bundleName: 'com.huawei.aios.agent.operation',
        abilityName: 'FloatWindowAbility',
        action: 'show',
      };
      const starter = ctx as ServiceExtensionStarter;
      if (starter.startServiceExtensionAbility) {
        await starter.startServiceExtensionAbility(want);
        return;
      }
      await ctx.startAbility(want);
    } catch (e) {
      console.error(`唤出全局浮窗失败: ${String(e)}`);
    }
  }

  private parseNonNegativeInt(text: string, fallback: number): number {
    const trimmed = text.trim();
    const parsed = parseInt(trimmed, 10);
    if (!Number.isFinite(parsed)) {
      return fallback;
    }
    return Math.max(0, parsed);
  }

  private async refreshHistory(): Promise<void> {
    this.historyError = '';
    this.historyLoading = true;
    try {
      const ctx = this.getUiContext();
      const store = TaskHistoryStore.getInstance();
      store.init(ctx);
      const tasks = await store.listTasks(DEFAULT_TASK_LIST_LIMIT);
      this.historyTasks = tasks.filter((t) => t.status !== 'running');
      if (this.selectedTaskId.length > 0) {
        const refreshed = await store.getTask(this.selectedTaskId);
        if (refreshed) {
          this.selectedTask = refreshed;
          this.selectedSteps = (await store.listSteps(this.selectedTaskId)).slice(0, DEFAULT_STEP_LIST_LIMIT);
          this.resetStepScreenshotCache(this.selectedSteps.length);
          if (this.selectedSteps.length > 0) {
            const target = this.selectedSteps.find((step) => step.stepIndex === this.selectedStepIndex);
            await this.selectStep(target ?? this.selectedSteps[0]);
          } else {
            this.selectedStep = null;
            this.selectedStepPixelMap = null;
            this.selectedStepParamLines = [];
            this.showStepMore = false;
            this.selectedStepIndex = 0;
            this.selectedStepPosition = 0;
          }
        }
      }
    } catch (e) {
      this.historyError = `加载历史失败: ${String(e)}`;
    } finally {
      this.historyLoading = false;
    }
  }

  private async openTask(taskId: string): Promise<void> {
    this.selectedTaskId = taskId;
    this.selectedStepIndex = 0;
    this.selectedStepPosition = 0;
    this.selectedStep = null;
    this.selectedStepPixelMap = null;
    this.selectedStepParamLines = [];
    this.resetStepScreenshotCache(0);
    this.showTaskMeta = false;
    this.showStepMore = false;
    try {
      const ctx = this.getUiContext();
      const store = TaskHistoryStore.getInstance();
      store.init(ctx);
      const task = await store.getTask(taskId);
      this.selectedTask = task;
      this.selectedSteps = task ? (await store.listSteps(taskId)).slice(0, DEFAULT_STEP_LIST_LIMIT) : [];
      this.resetStepScreenshotCache(this.selectedSteps.length);
      if (this.selectedSteps.length > 0) {
        await this.selectStep(this.selectedSteps[0]);
      }
    } catch (e) {
      this.historyError = `加载任务详情失败: ${String(e)}`;
      this.selectedTask = null;
      this.selectedSteps = [];
      this.resetStepScreenshotCache(0);
    }
  }

  private closeTask(): void {
    this.selectedTaskId = '';
    this.selectedTask = null;
    this.selectedSteps = [];
    this.selectedStepIndex = 0;
    this.selectedStepPosition = 0;
    this.selectedStep = null;
    this.selectedStepPixelMap = null;
    this.selectedStepParamLines = [];
    this.resetStepScreenshotCache(0);
    this.showTaskMeta = false;
    this.showStepMore = false;
  }

  private async selectStep(step: HistoryStepMeta): Promise<void> {
    this.selectStepAtPosition(this.findStepPosition(step.stepIndex));
  }

  private async deleteSelectedTask(): Promise<void> {
    const taskId = this.selectedTaskId.trim();
    if (taskId.length === 0) {
      return;
    }
    try {
      const ctx = this.getUiContext();
      const store = TaskHistoryStore.getInstance();
      store.init(ctx);
      await store.deleteTask(taskId);
      this.closeTask();
      await this.refreshHistory();
    } catch (e) {
      this.historyError = `删除失败: ${String(e)}`;
    }
  }

  private async deleteTaskDirect(taskId: string): Promise<void> {
    const id = taskId.trim();
    if (id.length === 0) {
      return;
    }
    try {
      const ctx = this.getUiContext();
      const store = TaskHistoryStore.getInstance();
      store.init(ctx);
      await store.deleteTask(id);
      if (this.selectedTaskId === id) {
        this.closeTask();
      }
      await this.refreshHistory();
    } catch (e) {
      this.historyError = `删除失败: ${String(e)}`;
    }
  }

  private async clearAllHistory(): Promise<void> {
    if (this.historyTasks.length === 0) {
      return;
    }
    try {
      const ctx = this.getUiContext();
      const store = TaskHistoryStore.getInstance();
      store.init(ctx);
      const tasks = await store.listTasks(DEFAULT_TASK_LIST_LIMIT);
      for (let i = 0; i < tasks.length; i++) {
        await store.deleteTask(tasks[i].taskId);
      }
      this.closeTask();
      await this.refreshHistory();
    } catch (e) {
      this.historyError = `清空失败: ${String(e)}`;
    }
  }

  private formatTime(ms: number): string {
    if (ms <= 0) {
      return '-';
    }
    try {
      const date = new Date(ms);
      return date.toLocaleString();
    } catch (_e) {
      return String(ms);
    }
  }

  private formatTimeShort(ms: number): string {
    if (ms <= 0) {
      return '-';
    }
    try {
      const date = new Date(ms);
      return date.toLocaleTimeString();
    } catch (_e) {
      return this.formatTime(ms);
    }
  }
}
