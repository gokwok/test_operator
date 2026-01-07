import { abilityAccessCtrl } from '@kit.AbilityKit';
import type { common, Permissions } from '@kit.AbilityKit';
import type { BusinessError } from '@kit.BasicServicesKit';
import image from '@ohos.multimedia.image';

import { TaskHistoryStore } from '../../features/persistence/history/TaskHistoryStore';
import type { HistoryStepMeta, HistoryTaskMeta } from '../../features/persistence/history/TaskHistoryStore';
import { AgentSettings } from '../../features/persistence/settings/AgentSettings';
import type { AgentSettingsData } from '../../features/persistence/settings/AgentSettings';
import { WallpaperState } from '../../features/agent_controller/state_collector/WallpaperState';
import { ForegroundAppState } from '../../features/agent_controller/state_collector/ForegroundAppState';

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
const STEP_IMAGE_HEIGHT: number = 460;
const STEP_SWIPER_HEIGHT: number = 540;
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

type NavSymbolName = 'house_fill' | 'gearshape' | 'bolt_filled_on_circle';
type TaskBarSymbolName = 'arrow_clockwise' | 'clean' | 'chevron_left' | 'trash';

@Entry
@Component
struct Index {
  @State activeTabIndex: number = 0;

  @State cloudEndpoint: string = '';
  @State waitMsText: string = '';
  @State liveViewEnabled: boolean = true;
  @State historyEnabled: boolean = true;
  @State historyMaxTasksText: string = '50';
  @State settingsInfo: string = '';
  @State wallpaperInfo: string = '';
  @State wallpaperPixelMap: image.PixelMap | null = null;
  @State foregroundAppInfo: string = '';

  @State historyLoading: boolean = false;
  @State historyError: string = '';
  @State historyTasks: Array<HistoryTaskMeta> = [];

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
  @State private viewportHeight: number = 0;

  private stepPixelMapRequestSeq: number = 0;
  private stepImageCacheSeq: number = 0;

  aboutToAppear(): void {
    void this.loadSettings();
    void this.refreshHistory();
    void this.ensureWriteImageVideoPermission();
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
        }, (task: HistoryTaskMeta) => task.taskId)
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
      .padding({ top: 10 })
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
    Column({ space: 10 }) {
      Row({ space: 8 }) {
        Text('步骤')
          .fontSize(DETAIL_TITLE_SIZE)
          .fontColor(THEME_TEXT_PRIMARY)
          .fontWeight(FontWeight.Bold)

        this.detailTag(`步骤 ${this.resolveStepSwiperIndex() + 1}/${this.selectedSteps.length}`, THEME_TEXT_PRIMARY, DETAIL_TAG_BG)
      }

      Text('左右滑动查看每一步的截图与详情')
        .fontSize(10)
        .fontColor(DETAIL_HINT_TEXT)

      Swiper() {
        ForEach(this.selectedSteps, (step: HistoryStepMeta, index: number) => {
          this.stepScreenshotSlide(step, index);
        }, (step: HistoryStepMeta) => `${step.taskId}:${step.stepIndex}`)
      }
      .loop(false)
      .indicator(true)
      .index(this.resolveStepSwiperIndex())
      .onChange((index: number) => {
        this.onStepChange(index);
      })
      .height(STEP_SWIPER_HEIGHT)
    }
    .width('100%')
  }

  @Builder
  private stepScreenshotSlide(step: HistoryStepMeta, position: number): void {
    Column({ space: 10 }) {
      this.stepScreenshotCard(step, position);

      Row({ space: 8 }) {
        Text(`第 ${position + 1} 步`)
          .fontSize(13)
          .fontColor(THEME_TEXT_PRIMARY)
          .fontWeight(FontWeight.Bold)

        this.detailTag(this.resolveActionDisplayName(step.actionName), DETAIL_TAG_TEXT, DETAIL_TAG_BG)
      }

      Text(step.description.trim().length > 0 ? step.description : '暂无步骤说明')
        .fontSize(12)
        .fontColor(THEME_TEXT_PRIMARY)
        .lineHeight(18)
        .maxLines(3)
        .textOverflow({ overflow: TextOverflow.Ellipsis })

      this.stepParamSection(step.actionJson)
    }
    .width('100%')
    .padding(14)
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
    const pixelMap =
      position >= 0 && position < this.stepPixelMaps.length ? this.stepPixelMaps[position] : null;
    const loading =
      position >= 0 && position < this.stepPixelMapLoading.length ? this.stepPixelMapLoading[position] : false;
    if (pixelMap) {
      Image(pixelMap)
        .width('100%')
        .height('100%')
        .backgroundColor(THEME_CARD)
        .objectFit(ImageFit.Contain)
    } else {
      Row() {
        Text(loading ? '正在加载截图…' : '暂无截图')
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
        this.detailTag(`第 ${this.resolveStepSwiperIndex() + 1} 步`, THEME_TEXT_PRIMARY, DETAIL_TAG_BG)
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

      this.statusCallout(
        '执行结果',
        this.resolveStepResultSummary(step),
        this.resolveStepResultBg(step),
        this.resolveStepResultBorder(step),
        this.resolveStepResultTextColor(step)
      )

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

        if (step.output.trim().length > 0) {
          this.detailSection('完整输出', step.output, THEME_TEXT_PRIMARY, 10)
        }

        if (step.error.trim().length > 0) {
          this.detailSection('错误详情', step.error, THEME_ERROR, 10)
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
        this.taskDeleteAction(task.taskId);
      }
    })
  }

  @Builder
  private taskDeleteAction(taskId: string): void {
    Row() {
      Image($r('app.media.delete'))
        .width(20)
        .height(20)
        .objectFit(ImageFit.Contain)
    }
    .width(72)
    .height('100%')
    .backgroundColor('transparent')
    .justifyContent(FlexAlign.Center)
    .alignItems(VerticalAlign.Center)
    .onClick(() => {
      void this.deleteTaskDirect(taskId);
    })
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
    if (normalized === 'task_over') {
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
    const lines: Array<StepParamLine> = this.buildStepParamLines(actionJson);
    if (lines.length > 0) {
      Column({ space: 6 }) {
        Text('关键参数')
          .fontSize(DETAIL_TITLE_SIZE)
          .fontColor(THEME_TEXT_PRIMARY)
          .fontWeight(FontWeight.Medium)

        ForEach(lines, (line: StepParamLine) => {
          this.detailRow(line.label, line.value)
        }, (line: StepParamLine) => line.label)
      }
      .padding(12)
      .borderRadius(DETAIL_SECTION_RADIUS)
      .backgroundColor(DETAIL_SECTION_BG)
      .border({ width: 1, color: DETAIL_SECTION_BORDER })
    }
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
          bottom: 0
        })
        .backgroundColor(THEME_PANEL)
        .borderRadius(26)
        .border({ width: this.resolvePanelBorderWidth(), color: THEME_BORDER })
      }
      .width('100%')
      .height('100%')

      this.bottomNav()
    }
    .width('100%')
    .height('100%')
    .padding({ left: 0, right: 0, top: SAFE_TOP_PADDING, bottom: 0 })
    .backgroundColor(THEME_BG_START)
    .onAreaChange((_oldValue, newValue) => {
      this.viewportHeight = Number(newValue.height);
    })
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

  private async loadSettings(): Promise<void> {
    this.settingsInfo = '';
    try {
      const ctx = this.getUiContext();
      const settings = await AgentSettings.load(ctx);
      this.cloudEndpoint = settings.cloudEndpoint;
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
