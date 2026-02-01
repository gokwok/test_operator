import type { common } from '@kit.AbilityKit';
import { formBindingData, formProvider } from '@kit.FormKit';
import { hilog } from '@kit.PerformanceAnalysisKit';

import { TaskHistoryStore } from '../history/TaskHistoryStore';
import type { HistoryTaskMeta } from '../history/TaskHistoryStore';
import { WidgetFormStore } from './WidgetFormStore';
import type { WidgetFormRecord } from './WidgetFormStore';

interface OperationWidgetFormData {
  title: string;
  shortcut1: string;
  shortcut2: string;
  shortcut3: string;
  shortcut4: string;
}

const OP_WIDGET_NAME: string = 'operation_widget';
const HILOG_DOMAIN: number = 0x0000;
const HILOG_TAG: string = 'OperationWidgetUpdater';

export class OperationWidgetUpdater {
  static async refreshOperationWidgetFavorites(ctx: common.Context): Promise<void> {
    const records = await WidgetFormStore.list(ctx);
    const formIds = OperationWidgetUpdater.pickOperationWidgetFormIds(records);
    if (formIds.length === 0) {
      hilog.warn(HILOG_DOMAIN, HILOG_TAG, 'refresh skipped: no formId records');
      return;
    }
    const favorites = await OperationWidgetUpdater.loadFavoriteSkills(ctx);
    hilog.info(
      HILOG_DOMAIN,
      HILOG_TAG,
      'refresh formIds=%{public}d favorites=%{public}d names=%{public}s',
      formIds.length,
      favorites.length,
      JSON.stringify(favorites)
    );
    const data = OperationWidgetUpdater.buildFavoritesData(favorites);
    for (let i = 0; i < formIds.length; i += 1) {
      const formId = formIds[i].trim();
      if (formId.length === 0) {
        continue;
      }
      try {
        const binding = OperationWidgetUpdater.createFormBindingDataWithProxies(data);
        formProvider.updateForm(formId, binding);
      } catch {
        hilog.warn(HILOG_DOMAIN, HILOG_TAG, 'updateForm failed formId=%{public}s', formId);
      }
    }
  }

  private static createFormBindingDataWithProxies(data: OperationWidgetFormData): formBindingData.FormBindingData {
    const binding = formBindingData.createFormBindingData(data);
    const proxies: Array<Object> = [];
    try {
      Reflect.set(binding as Object, 'formDataProxies', proxies);
    } catch (_e: Object) {
      // ignore
    }
    return binding;
  }

  private static pickOperationWidgetFormIds(records: Array<WidgetFormRecord>): Array<string> {
    const ids: Array<string> = [];
    for (let i = 0; i < records.length; i += 1) {
      const record = records[i];
      if (record.widgetName !== OP_WIDGET_NAME) {
        continue;
      }
      const id = record.formId.trim();
      if (id.length > 0 && id !== 'undefined' && id !== 'null') {
        ids.push(id);
      }
    }
    return ids;
  }

  private static async loadFavoriteSkills(ctx: common.Context): Promise<Array<string>> {
    const store = TaskHistoryStore.getInstance();
    store.init(ctx);
    const tasks = await store.listTasks(200);
    const favorites: Array<HistoryTaskMeta> = tasks.filter((t) => {
      const name = typeof t.skill_name === 'string' ? t.skill_name : '';
      return name.trim().length > 0 && t.status !== 'running';
    });
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

  private static buildFavoritesData(favorites: Array<string>): OperationWidgetFormData {
    const data: OperationWidgetFormData = {
      title: '',
      shortcut1: favorites.length >= 1 ? favorites[0] : '快捷键1',
      shortcut2: favorites.length >= 2 ? favorites[1] : '快捷键2',
      shortcut3: favorites.length >= 3 ? favorites[2] : '快捷键3',
      shortcut4: favorites.length >= 4 ? favorites[3] : '快捷键4',
    };
    return data;
  }
}
