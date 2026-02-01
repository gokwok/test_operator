import type { common } from '@kit.AbilityKit';

import { TaskHistoryStore } from '../../features/persistence/history/TaskHistoryStore';
import type { HistoryTaskMeta } from '../../features/persistence/history/TaskHistoryStore';

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

  aboutToAppear(): void {
    void this.refreshFavoritesBestEffort();
  }

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

  private async refreshFavoritesBestEffort(): Promise<void> {
    try {
      const ctx = getContext(this) as common.Context;
      const favorites = await this.loadFavoriteSkills(ctx);
      this.shortcut1 = favorites.length >= 1 ? favorites[0] : '快捷键1';
      this.shortcut2 = favorites.length >= 2 ? favorites[1] : '快捷键2';
      this.shortcut3 = favorites.length >= 3 ? favorites[2] : '快捷键3';
      this.shortcut4 = favorites.length >= 4 ? favorites[3] : '快捷键4';
    } catch (_e) {
      // ignore
    }
  }

  private async loadFavoriteSkills(ctx: common.Context): Promise<Array<string>> {
    const store = TaskHistoryStore.getInstance();
    store.init(ctx);
    const tasks = await store.listTasks(200);
    const favorites: Array<HistoryTaskMeta> = [];
    for (let i = 0; i < tasks.length; i += 1) {
      const task = tasks[i];
      const name = typeof task.skill_name === 'string' ? task.skill_name.trim() : '';
      if (name.length === 0) {
        continue;
      }
      if (task.status === 'running') {
        continue;
      }
      favorites.push(task);
    }
    favorites.sort((a, b) => {
      const aTs = a.endAtMs > 0 ? a.endAtMs : a.startAtMs;
      const bTs = b.endAtMs > 0 ? b.endAtMs : b.startAtMs;
      return bTs - aTs;
    });
    const picked: Array<string> = [];
    for (let i = 0; i < favorites.length && picked.length < 4; i += 1) {
      const name = favorites[i].skill_name.trim();
      if (name.length > 0) {
        picked.push(name);
      }
    }
    return picked;
  }
}
