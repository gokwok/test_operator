import type { common } from '@kit.AbilityKit';
import image from '@ohos.multimedia.image';

import { TaskHistoryStore } from '../../features/persistence/history/TaskHistoryStore';
import type { HistoryStepMeta, HistoryTaskMeta } from '../../features/persistence/history/TaskHistoryStore';
import { AgentSettings } from '../../features/persistence/settings/AgentSettings';
import type { AgentSettingsData } from '../../features/persistence/settings/AgentSettings';
import { WallpaperState } from '../../features/agent_controller/state_collector/WallpaperState';

const DEFAULT_TASK_LIST_LIMIT: number = 100;
const DEFAULT_STEP_LIST_LIMIT: number = 200;

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
          .fontSize(12)
          .fontColor(Color.White)
          .fontWeight(FontWeight.Medium)

        TextInput({ placeholder: '云侧地址（http://ip:port）', text: this.cloudEndpoint })
          .height(32)
          .fontSize(12)
          .fontColor(Color.White)
          .backgroundColor('#1A1B22')
          .borderRadius(10)
          .padding({ left: 10, right: 10 })
          .onChange((value: string) => {
            this.cloudEndpoint = value;
          })

        Row({ space: 8 }) {
          TextInput({ placeholder: 'wait_ms (动作后等待)', text: this.waitMsText })
            .layoutWeight(1)
            .height(32)
            .fontSize(12)
            .fontColor(Color.White)
            .backgroundColor('#1A1B22')
            .borderRadius(10)
            .padding({ left: 10, right: 10 })
            .onChange((value: string) => {
              this.waitMsText = value;
            })

          Button(this.liveViewEnabled ? 'LiveView: 开' : 'LiveView: 关')
            .fontSize(12)
            .fontColor(Color.White)
            .backgroundColor('#2A2C34')
            .borderRadius(14)
            .padding({ left: 12, right: 12, top: 6, bottom: 6 })
            .onClick(() => {
              this.liveViewEnabled = !this.liveViewEnabled;
            })
        }

        Row({ space: 8 }) {
          Button(this.historyEnabled ? '历史: 开' : '历史: 关')
            .fontSize(12)
            .fontColor(Color.White)
            .backgroundColor('#2A2C34')
            .borderRadius(14)
            .padding({ left: 12, right: 12, top: 6, bottom: 6 })
            .onClick(() => {
              this.historyEnabled = !this.historyEnabled;
            })

          TextInput({ placeholder: '最多保留任务数', text: this.historyMaxTasksText })
            .layoutWeight(1)
            .height(32)
            .fontSize(12)
            .fontColor(Color.White)
            .backgroundColor('#1A1B22')
            .borderRadius(10)
            .padding({ left: 10, right: 10 })
            .onChange((value: string) => {
              this.historyMaxTasksText = value;
            })
        }

        Row({ space: 8 }) {
          Button('保存配置')
            .fontSize(12)
            .fontColor(Color.White)
            .backgroundColor('#2A2C34')
            .borderRadius(14)
            .padding({ left: 12, right: 12, top: 6, bottom: 6 })
            .onClick(() => {
              void this.saveSettings();
            })

          Button('刷新历史')
            .fontSize(12)
            .fontColor(Color.White)
            .backgroundColor('#2A2C34')
            .borderRadius(14)
            .padding({ left: 12, right: 12, top: 6, bottom: 6 })
            .onClick(() => {
              void this.refreshHistory();
            })
        }

        if (this.settingsInfo.length > 0) {
          Text(this.settingsInfo)
            .fontSize(11)
            .fontColor('#B8C1CC')
        }

        Text('壁纸调试')
          .fontSize(12)
          .fontColor(Color.White)
          .fontWeight(FontWeight.Medium)

        Row({ space: 8 }) {
          Button('获取壁纸')
            .fontSize(12)
            .fontColor(Color.White)
            .backgroundColor('#2A2C34')
            .borderRadius(14)
            .padding({ left: 12, right: 12, top: 6, bottom: 6 })
            .onClick(() => {
              void this.loadWallpaper();
            })
        }

        if (this.wallpaperInfo.length > 0) {
          Text(this.wallpaperInfo)
            .fontSize(11)
            .fontColor('#B8C1CC')
        }

        if (this.wallpaperPixelMap) {
          Image(this.wallpaperPixelMap)
            .width('100%')
            .height(180)
            .borderRadius(12)
            .backgroundColor('#111217')
            .objectFit(ImageFit.Contain)
        }
      }
      .width('100%')
    }
    .scrollBar(BarState.On)
  }

  @Builder
  private historyTab(): void {
    Scroll() {
      Column({ space: 12 }) {
        Row({ space: 8 }) {
          Text('历史任务')
            .fontSize(12)
            .fontColor(Color.White)
            .fontWeight(FontWeight.Medium)
            .layoutWeight(1)

          Button('刷新')
            .fontSize(12)
            .fontColor(Color.White)
            .backgroundColor('#2A2C34')
            .borderRadius(14)
            .padding({ left: 12, right: 12, top: 6, bottom: 6 })
            .onClick(() => {
              void this.refreshHistory();
            })

          Button('清空')
            .fontSize(12)
            .fontColor(Color.White)
            .backgroundColor('#2A2C34')
            .borderRadius(14)
            .padding({ left: 12, right: 12, top: 6, bottom: 6 })
            .onClick(() => {
              void this.clearAllHistory();
            })
        }

        if (this.historyLoading) {
          Text('加载中...')
            .fontSize(11)
            .fontColor('#B8C1CC')
        }

        if (this.historyError.length > 0) {
          Text(this.historyError)
            .fontSize(11)
            .fontColor('#F8B4B4')
        }

        if (this.selectedTask && this.selectedTaskId.length > 0) {
          this.taskDetailView();
        } else {
          this.taskListView();
        }
      }
      .width('100%')
    }
    .scrollBar(BarState.On)
  }

  @Builder
  private taskListView(): void {
    if (this.historyTasks.length === 0) {
      Text('暂无历史任务')
        .fontSize(11)
        .fontColor('#B8C1CC')
    } else {
      List({ space: 10 }) {
        ForEach(this.historyTasks, (task: HistoryTaskMeta) => {
          ListItem() {
            Column({ space: 6 }) {
              Row() {
                Text(this.formatTime(task.startAtMs))
                  .fontSize(11)
                  .fontColor('#B8C1CC')
                  .layoutWeight(1)

                Text(task.status)
                  .fontSize(11)
                  .fontColor('#B8C1CC')
              }

              Text(task.taskText)
                .fontSize(12)
                .fontColor(Color.White)
                .maxLines(2)
                .textOverflow({ overflow: TextOverflow.Ellipsis })

              Row() {
                Text(`steps: ${task.stepCount}`)
                  .fontSize(11)
                  .fontColor('#B8C1CC')
                  .layoutWeight(1)

                if (task.endAtMs > 0) {
                  Text(`end: ${this.formatTime(task.endAtMs)}`)
                    .fontSize(11)
                    .fontColor('#B8C1CC')
                }
              }
            }
            .padding(12)
            .borderRadius(12)
            .backgroundColor('#111217')
            .onClick(() => {
              void this.openTask(task.taskId);
            })
          }
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
            .fontColor(Color.White)
            .backgroundColor('#2A2C34')
            .borderRadius(14)
            .padding({ left: 12, right: 12, top: 6, bottom: 6 })
            .onClick(() => {
              this.closeTask();
            })

          Button('删除')
            .fontSize(12)
            .fontColor(Color.White)
            .backgroundColor('#2A2C34')
            .borderRadius(14)
            .padding({ left: 12, right: 12, top: 6, bottom: 6 })
            .onClick(() => {
              void this.deleteSelectedTask();
            })

          Text(this.selectedTask.status)
            .fontSize(11)
            .fontColor('#B8C1CC')
            .layoutWeight(1)
            .textAlign(TextAlign.End)
        }

        Text(this.selectedTask.taskText)
          .fontSize(12)
          .fontColor(Color.White)

        Text(`session: ${this.selectedTask.sessionId}`)
          .fontSize(11)
          .fontColor('#B8C1CC')

        Text(`cloud: ${this.selectedTask.cloudEndpoint}`)
          .fontSize(11)
          .fontColor('#B8C1CC')

        Text(`wait_ms: ${this.selectedTask.waitAfterActionMs}    steps: ${this.selectedTask.stepCount}`)
          .fontSize(11)
          .fontColor('#B8C1CC')

        if (this.selectedTask.finishContent.trim().length > 0) {
          Text(`finish:\n${this.selectedTask.finishContent}`)
            .fontSize(11)
            .fontColor('#B8C1CC')
        } else if (this.selectedTask.error.trim().length > 0) {
          Text(`error:\n${this.selectedTask.error}`)
            .fontSize(11)
            .fontColor('#F8B4B4')
        }

        if (this.selectedStepPixelMap) {
          Image(this.selectedStepPixelMap)
            .width('100%')
            .height(280)
            .borderRadius(12)
            .backgroundColor('#111217')
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
        .fontColor('#B8C1CC')
    } else {
      List({ space: 8 }) {
        ForEach(this.selectedSteps, (step: HistoryStepMeta) => {
          ListItem() {
            Column({ space: 6 }) {
              Row() {
                Text(`#${step.stepIndex}  ${step.actionName}`)
                  .fontSize(12)
                  .fontColor(Color.White)
                  .layoutWeight(1)

                Text(step.resultStatus.length > 0 ? step.resultStatus : '-')
                  .fontSize(11)
                  .fontColor('#B8C1CC')
              }

              if (step.description.trim().length > 0) {
                Text(step.description)
                  .fontSize(11)
                  .fontColor('#B8C1CC')
                  .maxLines(2)
                  .textOverflow({ overflow: TextOverflow.Ellipsis })
              }
            }
            .padding(12)
            .borderRadius(12)
            .backgroundColor(this.selectedStepIndex === step.stepIndex ? '#1A1B22' : '#111217')
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
          .fontColor(Color.White)
          .fontWeight(FontWeight.Medium)

        if (this.selectedStep.error.trim().length > 0) {
          Text(this.selectedStep.error)
            .fontSize(11)
            .fontColor('#F8B4B4')
        }

        if (this.selectedStep.output.trim().length > 0) {
          Text(this.selectedStep.output)
            .fontSize(11)
            .fontColor('#B8C1CC')
        }
      }
      .padding(12)
      .borderRadius(12)
      .backgroundColor('#111217')
      .width('100%')
    }
  }

  build() {
    Column() {
      Tabs({ index: this.activeTabIndex }) {
        TabContent() {
          this.settingsTab();
        }
        .tabBar('设置')

        TabContent() {
          this.historyTab();
        }
        .tabBar('历史')
      }
      .barPosition(BarPosition.Start)
      .scrollable(true)
      .onChange((index: number) => {
        this.activeTabIndex = index;
      })
      .width('100%')
      .height('100%')
      .padding({ left: 16, right: 16, top: 16, bottom: 24 })
    }
    .width('100%')
    .height('100%')
    .backgroundColor('#090A0F')
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
