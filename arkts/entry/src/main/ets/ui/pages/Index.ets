import type { common } from '@kit.AbilityKit';
import image from '@ohos.multimedia.image';

import { TaskHistoryStore } from '../../features/persistence/history/TaskHistoryStore';
import type { HistoryStepMeta, HistoryTaskMeta } from '../../features/persistence/history/TaskHistoryStore';
import { AgentSettings } from '../../features/persistence/settings/AgentSettings';
import type { AgentSettingsData } from '../../features/persistence/settings/AgentSettings';
import { WallpaperState } from '../../features/agent_controller/state_collector/WallpaperState';

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
const NAV_HEIGHT: number = 72;
const NAV_BLUR_RADIUS: number = 18;
const TASK_BAR_HEIGHT: number = 48;
const TASK_BAR_ICON_SIZE: number = 18;
const TASK_BAR_ACTION_WIDTH: number = 76;
const TASK_CARD_SUCCESS_BG: string = '#EAF8EE';
const TASK_CARD_SUCCESS_BORDER: string = '#6FBF88';
const TASK_CARD_FAIL_BG: string = '#FCEAEA';
const TASK_CARD_FAIL_BORDER: string = '#E18181';
const TASK_CARD_ICON_SIZE: number = 18;
const TASK_CARD_MIN_HEIGHT: number = 132;

type NavSymbolName = 'house_fill' | 'gearshape' | 'bolt_filled_on_circle';
type TaskBarSymbolName = 'arrow_clockwise' | 'clean';

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

  @State historyLoading: boolean = false;
  @State historyError: string = '';
  @State historyTasks: Array<HistoryTaskMeta> = [];

  @State selectedTaskId: string = '';
  @State selectedTask: HistoryTaskMeta | null = null;
  @State selectedSteps: Array<HistoryStepMeta> = [];
  @State selectedStepIndex: number = 0;
  @State selectedStep: HistoryStepMeta | null = null;
  @State selectedStepPixelMap: image.PixelMap | null = null;

  aboutToAppear(): void {
    void this.loadSettings();
    void this.refreshHistory();
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
        }

        if (this.wallpaperInfo.length > 0) {
          Text(this.wallpaperInfo)
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
        this.taskTopBar();

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
        }
        .width('100%')
        .padding({ left: 16, right: 16, bottom: 12 })
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
      Column({ space: 12 }) {
        Row({ space: 8 }) {
          Button('返回')
            .fontSize(12)
            .fontColor(THEME_TEXT_PRIMARY)
            .backgroundColor(THEME_CARD)
            .borderRadius(14)
            .border({ width: 1, color: THEME_BORDER })
            .padding({ left: 12, right: 12, top: 6, bottom: 6 })
            .onClick(() => {
              this.closeTask();
            })

          Button('删除')
            .fontSize(12)
            .fontColor(THEME_ERROR)
            .backgroundColor('#FFF2F2')
            .borderRadius(14)
            .border({ width: 1, color: '#F3C6C6' })
            .padding({ left: 12, right: 12, top: 6, bottom: 6 })
            .onClick(() => {
              void this.deleteSelectedTask();
            })

          Text(this.selectedTask.status)
            .fontSize(11)
            .fontColor(THEME_TEXT_MUTED)
            .layoutWeight(1)
            .textAlign(TextAlign.End)
        }

        Text(this.selectedTask.taskText)
          .fontSize(12)
          .fontColor(THEME_TEXT_PRIMARY)

        Text(`session: ${this.selectedTask.sessionId}`)
          .fontSize(11)
          .fontColor(THEME_TEXT_MUTED)

        Text(`cloud: ${this.selectedTask.cloudEndpoint}`)
          .fontSize(11)
          .fontColor(THEME_TEXT_MUTED)

        Text(`wait_ms: ${this.selectedTask.waitAfterActionMs}    steps: ${this.selectedTask.stepCount}`)
          .fontSize(11)
          .fontColor(THEME_TEXT_MUTED)

        if (this.selectedTask.finishContent.trim().length > 0) {
          Text(`finish:\n${this.selectedTask.finishContent}`)
            .fontSize(11)
            .fontColor(THEME_TEXT_MUTED)
        } else if (this.selectedTask.error.trim().length > 0) {
          Text(`error:\n${this.selectedTask.error}`)
            .fontSize(11)
            .fontColor(THEME_ERROR)
        }

        if (this.selectedStepPixelMap) {
          Image(this.selectedStepPixelMap)
            .width('100%')
            .height(280)
            .borderRadius(12)
            .backgroundColor(THEME_CARD)
            .objectFit(ImageFit.Contain)
        }

        if (this.selectedStep) {
          this.stepDetailView();
        }

        this.stepListView()
      }
      .width('100%')
      .padding({ top: 8 })
    }
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
      this.historyTab();
    } else if (this.activeTabIndex === 1) {
      this.debugTab();
    } else {
      this.settingsTab();
    }
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
    .justifyContent(FlexAlign.Center)
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

  @Builder
  private taskTopBar(): void {
    Row() {
      Row()
        .width(TASK_BAR_ACTION_WIDTH)
        .height('100%')

      Text('任务')
        .fontSize(15)
        .fontFamily(FONT_DISPLAY)
        .fontColor(THEME_TEXT_PRIMARY)
        .fontWeight(FontWeight.Medium)
        .layoutWeight(1)
        .textAlign(TextAlign.Center)

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
    .width('100%')
    .height(TASK_BAR_HEIGHT)
    .backgroundColor(Color.White)
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
    return $r('sys.symbol.clean');
  }

  @Builder
  private taskCardItem(task: HistoryTaskMeta): void {
    ListItem() {
      Column({ space: 10 }) {
        Row({ space: 8 }) {
          Text(task.status)
            .fontSize(11)
            .fontColor(this.resolveTaskBorderColor(task.status))
            .fontWeight(FontWeight.Bold)
            .layoutWeight(1)

          Image(this.resolveTaskStatusIcon(task.status))
            .width(TASK_CARD_ICON_SIZE)
            .height(TASK_CARD_ICON_SIZE)
            .objectFit(ImageFit.Contain)
        }

        Text(task.taskText)
          .fontSize(14)
          .fontColor(THEME_TEXT_PRIMARY)
          .fontWeight(FontWeight.Medium)
          .maxLines(3)
          .textOverflow({ overflow: TextOverflow.Ellipsis })

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
      .padding(14)
      .minHeight(TASK_CARD_MIN_HEIGHT)
      .borderRadius(16)
      .backgroundColor(this.resolveTaskBackground(task.status))
      .border({ width: 1, color: this.resolveTaskBorderColor(task.status) })
    }
    .onClick(() => {
      void this.openTask(task.taskId);
    })
    .swipeAction({
      start: () => {
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
    .backgroundColor('#FFECEC')
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

  build() {
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
        bottom: 12
      })
      .backgroundColor(THEME_PANEL)
      .borderRadius(26)
      .border({ width: this.resolvePanelBorderWidth(), color: THEME_BORDER })

      this.bottomNav();
    }
    .width('100%')
    .height('100%')
    .padding({ left: 0, right: 0, top: SAFE_TOP_PADDING, bottom: 0 })
    .backgroundColor(THEME_BG_START)
  }

  private getUiContext(): common.UIAbilityContext {
    return getContext(this) as common.UIAbilityContext;
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
    this.selectedStep = null;
    this.selectedStepPixelMap = null;
    try {
      const ctx = this.getUiContext();
      const store = TaskHistoryStore.getInstance();
      store.init(ctx);
      const task = await store.getTask(taskId);
      this.selectedTask = task;
      this.selectedSteps = task ? (await store.listSteps(taskId)).slice(0, DEFAULT_STEP_LIST_LIMIT) : [];
    } catch (e) {
      this.historyError = `加载任务详情失败: ${String(e)}`;
      this.selectedTask = null;
      this.selectedSteps = [];
    }
  }

  private closeTask(): void {
    this.selectedTaskId = '';
    this.selectedTask = null;
    this.selectedSteps = [];
    this.selectedStepIndex = 0;
    this.selectedStep = null;
    this.selectedStepPixelMap = null;
  }

  private async selectStep(step: HistoryStepMeta): Promise<void> {
    this.selectedStepIndex = step.stepIndex;
    this.selectedStep = step;
    this.selectedStepPixelMap = null;
    try {
      const ctx = this.getUiContext();
      const store = TaskHistoryStore.getInstance();
      store.init(ctx);
      const pixelMap = await store.getStepPixelMap(step.taskId, step.stepIndex);
      this.selectedStepPixelMap = pixelMap;
    } catch (e) {
      this.historyError = `加载截图失败: ${String(e)}`;
    }
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
}
