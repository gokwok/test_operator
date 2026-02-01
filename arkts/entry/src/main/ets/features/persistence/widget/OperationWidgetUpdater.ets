import type { common } from '@kit.AbilityKit';
import { formBindingData, formProvider } from '@kit.FormKit';
import { hilog } from '@kit.PerformanceAnalysisKit';

import { FavoriteSkillPicker } from './FavoriteSkillPicker';
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
    let favorites: Array<string> = [];
    try {
      favorites = await FavoriteSkillPicker.pick(ctx, 4);
    } catch (e) {
      hilog.warn(HILOG_DOMAIN, HILOG_TAG, 'load favorites failed: %{public}s', String(e));
      favorites = [];
    }
    hilog.info(
      HILOG_DOMAIN,
      HILOG_TAG,
      'refresh formIds=%{public}d favorites=%{public}d ids=%{public}s names=%{public}s',
      formIds.length,
      favorites.length,
      JSON.stringify(formIds),
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
        await formProvider.updateForm(formId, binding);
        hilog.info(HILOG_DOMAIN, HILOG_TAG, 'updateForm ok formId=%{public}s', formId);
      } catch (e) {
        hilog.warn(HILOG_DOMAIN, HILOG_TAG, 'updateForm failed formId=%{public}s err=%{public}s', formId, String(e));
      }
      OperationWidgetUpdater.requestRefreshBestEffort(formId);
    }
  }

  private static createFormBindingDataWithProxies(data: OperationWidgetFormData): formBindingData.FormBindingData {
    return formBindingData.createFormBindingData(data);
  }

  private static requestRefreshBestEffort(formId: string): void {
    // Some SDKs expose request-update APIs under formProvider, but ArkTS doesn't allow
    // reflection on namespaces. We rely on updateForm/updateDuration instead.
    void formId;
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
