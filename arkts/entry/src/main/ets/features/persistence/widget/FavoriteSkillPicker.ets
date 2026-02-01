import type { common } from '@kit.AbilityKit';

import { TaskHistoryStore } from '../history/TaskHistoryStore';
import type { HistoryTaskMeta } from '../history/TaskHistoryStore';

export class FavoriteSkillPicker {
  static async pick(ctx: common.Context, limit: number = 4): Promise<Array<string>> {
    const capped = Number.isFinite(limit) ? Math.max(0, Math.floor(limit)) : 0;
    if (capped <= 0) {
      return [];
    }
    const store = TaskHistoryStore.getInstance();
    store.init(ctx);
    const tasks = await store.listTasks(200);
    const favorites: Array<HistoryTaskMeta> = tasks.filter((t) => FavoriteSkillPicker.isFavoriteTask(t));
    favorites.sort((a, b) => FavoriteSkillPicker.sortByRecent(a, b));

    const picked: Array<string> = [];
    for (let i = 0; i < favorites.length && picked.length < capped; i += 1) {
      const name = favorites[i].skill_name.trim();
      if (name.length === 0) {
        continue;
      }
      if (FavoriteSkillPicker.contains(picked, name)) {
        continue;
      }
      picked.push(name);
    }
    return picked;
  }

  private static isFavoriteTask(task: HistoryTaskMeta): boolean {
    const name = typeof task.skill_name === 'string' ? task.skill_name : '';
    return name.trim().length > 0 && task.status !== 'running';
  }

  private static sortByRecent(a: HistoryTaskMeta, b: HistoryTaskMeta): number {
    const aTs = a.endAtMs > 0 ? a.endAtMs : a.startAtMs;
    const bTs = b.endAtMs > 0 ? b.endAtMs : b.startAtMs;
    return bTs - aTs;
  }

  private static contains(list: Array<string>, value: string): boolean {
    for (let i = 0; i < list.length; i += 1) {
      if (list[i] === value) {
        return true;
      }
    }
    return false;
  }
}
