import type { common, Want } from '@kit.AbilityKit';
import { FormExtensionAbility, formBindingData, formProvider } from '@kit.FormKit';
import { hilog } from '@kit.PerformanceAnalysisKit';

import { FavoriteSkillPicker } from '../features/persistence/widget/FavoriteSkillPicker';
import { WidgetFormStore, WidgetFormRecord } from '../features/persistence/widget/WidgetFormStore';
import { AgentControllerClient } from '../features/agent_controller/ipc/AgentControllerClient';

const HILOG_DOMAIN: number = 0x0000;
const HILOG_TAG: string = 'OperationFormAbility';
const OP_WIDGET_NAME: string = 'operation_widget';
const OP_WIDGET_DIMENSION: string = '2*4';

interface OperationWidgetFormData {
  title: string;
  shortcut1: string;
  shortcut2: string;
  shortcut3: string;
  shortcut4: string;
}

interface OperationWidgetEventPayload {
  type?: string;
  skill_name?: string;
}

export default class OperationFormAbility extends FormExtensionAbility {
  onAddForm(want: Want): formBindingData.FormBindingData {
    hilog.info(HILOG_DOMAIN, HILOG_TAG, 'onAddForm want=%{public}s', JSON.stringify(want));
    const formId = this.resolveFormId(want);
    if (formId.length > 0) {
      void this.updateFormFavoritesBestEffort(formId);
    }
    void this.trackFormAdded(want);
    return this.createFormBindingDataWithProxies(this.buildDefaultData());
  }

  onUpdateForm(formId: string): void {
    hilog.info(HILOG_DOMAIN, HILOG_TAG, 'onUpdateForm formId=%{public}s', formId);
    void this.updateFormFavoritesBestEffort(formId);
    void this.trackFormUpdated(formId);
  }

  onRemoveForm(formId: string): void {
    hilog.info(HILOG_DOMAIN, HILOG_TAG, 'onRemoveForm formId=%{public}s', formId);
    void this.trackFormRemoved(formId);
  }

  onFormEvent(formId: string, message: string): void {
    hilog.info(HILOG_DOMAIN, HILOG_TAG, 'onFormEvent formId=%{public}s message=%{public}s', formId, message);
    const skillName = this.extractSkillName(message);
    if (skillName.length === 0) {
      return;
    }
    void this.replaySkillBestEffort(skillName);
    void this.updateFormFavoritesBestEffort(formId);
  }

  private buildDefaultData(): OperationWidgetFormData {
    const data: OperationWidgetFormData = {
      title: '',
      shortcut1: '快捷键1',
      shortcut2: '快捷键2',
      shortcut3: '快捷键3',
      shortcut4: '快捷键4',
    };
    return data;
  }

  private async updateFormFavoritesBestEffort(formId: string): Promise<void> {
    const ctx = this.context;
    const trimmed = this.sanitizeFormId(formId);
    if (!ctx || trimmed.length === 0) {
      if (!ctx) {
        hilog.warn(HILOG_DOMAIN, HILOG_TAG, 'updateFormFavorites skipped: context missing');
      } else {
        hilog.warn(HILOG_DOMAIN, HILOG_TAG, 'updateFormFavorites skipped: formId empty raw=%{public}s', formId);
      }
      return;
    }
    try {
      const favorites = await this.loadFavoriteSkills(ctx);
      hilog.info(
        HILOG_DOMAIN,
        HILOG_TAG,
        'updateFormFavorites formId=%{public}s count=%{public}d names=%{public}s',
        trimmed,
        favorites.length,
        JSON.stringify(favorites)
      );
      const data = this.buildFavoritesData(favorites);
      await formProvider.updateForm(trimmed, this.createFormBindingDataWithProxies(data));
      hilog.info(HILOG_DOMAIN, HILOG_TAG, 'updateForm ok formId=%{public}s', trimmed);
    } catch (e) {
      hilog.warn(HILOG_DOMAIN, HILOG_TAG, 'updateFormFavorites failed: %{public}s', String(e));
    }
  }

  private createFormBindingDataWithProxies(data: OperationWidgetFormData): formBindingData.FormBindingData {
    return formBindingData.createFormBindingData(data);
  }

  private async loadFavoriteSkills(ctx: common.Context): Promise<Array<string>> {
    return await FavoriteSkillPicker.pick(ctx, 4);
  }

  private buildFavoritesData(favorites: Array<string>): OperationWidgetFormData {
    const fallback = this.buildDefaultData();
    const data: OperationWidgetFormData = {
      title: fallback.title,
      shortcut1: favorites.length >= 1 ? favorites[0] : fallback.shortcut1,
      shortcut2: favorites.length >= 2 ? favorites[1] : fallback.shortcut2,
      shortcut3: favorites.length >= 3 ? favorites[2] : fallback.shortcut3,
      shortcut4: favorites.length >= 4 ? favorites[3] : fallback.shortcut4,
    };
    return data;
  }

  private extractSkillName(message: string): string {
    const trimmed = message.trim();
    if (trimmed.length === 0) {
      return '';
    }
    if (trimmed.startsWith('{')) {
      try {
        const parsed = JSON.parse(trimmed) as OperationWidgetEventPayload;
        const type = typeof parsed?.type === 'string' ? parsed.type : '';
        const skill = typeof parsed?.skill_name === 'string' ? parsed.skill_name : '';
        if (type.trim() === 'replay_skill') {
          return skill.trim();
        }
      } catch (_e) {
        // ignore
      }
      return '';
    }
    return trimmed;
  }

  private async replaySkillBestEffort(skillName: string): Promise<void> {
    try {
      const ctx = this.context;
      if (!ctx) {
        return;
      }
      const client = new AgentControllerClient();
      await client.connectFromExtension(ctx as common.UIExtensionContext);
      await client.replaySkill(skillName);
      await client.disconnectFromExtension(ctx as common.UIExtensionContext);
    } catch (e) {
      hilog.warn(HILOG_DOMAIN, HILOG_TAG, 'replaySkill failed: %{public}s', String(e));
    }
  }

  private async trackFormAdded(want: Want): Promise<void> {
    try {
      const ctx = this.context;
      const formId = this.resolveFormId(want);
      if (!ctx || formId.length === 0) {
        hilog.warn(HILOG_DOMAIN, HILOG_TAG, 'trackFormAdded skipped: formId empty');
        return;
      }
      const record: WidgetFormRecord = {
        formId,
        widgetName: OP_WIDGET_NAME,
        dimension: OP_WIDGET_DIMENSION,
        createdAtMs: Date.now(),
        updatedAtMs: Date.now(),
      };
      await WidgetFormStore.upsert(ctx, record);
    } catch (e) {
      hilog.warn(HILOG_DOMAIN, HILOG_TAG, 'trackFormAdded failed: %{public}s', String(e));
    }
  }

  private async trackFormUpdated(formId: string): Promise<void> {
    try {
      const ctx = this.context;
      const trimmed = this.sanitizeFormId(formId);
      if (!ctx || trimmed.length === 0) {
        return;
      }
      const record: WidgetFormRecord = {
        formId: trimmed,
        widgetName: OP_WIDGET_NAME,
        dimension: OP_WIDGET_DIMENSION,
        createdAtMs: Date.now(),
        updatedAtMs: Date.now(),
      };
      await WidgetFormStore.upsert(ctx, record);
    } catch (_e) {
      // ignore
    }
  }

  private async trackFormRemoved(formId: string): Promise<void> {
    try {
      const ctx = this.context;
      if (!ctx) {
        return;
      }
      await WidgetFormStore.remove(ctx, formId);
    } catch (_e) {
      // ignore
    }
  }

  private resolveFormId(want: Want): string {
    const params = want.parameters as Object;
    const formId = this.readFirstParamText(params, [
      'formId',
      'form_id',
      'formID',
      'form_identity',
      'ohos.extra.param.key.form_id',
      'ohos.extra.param.key.formId',
      'ohos.extra.param.key.FORM_ID',
      'ohos.extra.param.key.form_identity',
      'ohos.extra.form_id',
      'ohos.extra.formId',
    ]);
    if (formId.length > 0) {
      return this.sanitizeFormId(formId);
    }
    const raw = this.safeStringify(want as Object);
    if (raw.length === 0) {
      return '';
    }
    const extracted = this.extractFirstJsonField(raw, [
      'formId',
      'form_id',
      'form_identity',
      'ohos.extra.param.key.form_id',
      'ohos.extra.param.key.form_identity',
      'ohos.extra.form_id',
    ]);
    return this.sanitizeFormId(extracted);
  }

  private readFirstParamText(params: Object, keys: Array<string>): string {
    if (!params || typeof params !== 'object') {
      return '';
    }
    for (let i = 0; i < keys.length; i += 1) {
      const key = keys[i];
      const value = this.safeGet(params, key);
      const text = this.normalizeText(value);
      if (text.length > 0) {
        return text;
      }
    }
    return '';
  }

  private extractFirstJsonField(raw: string, keys: Array<string>): string {
    for (let i = 0; i < keys.length; i += 1) {
      const key = keys[i];
      const escaped = key.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
      const pattern = new RegExp(`\"${escaped}\"\\s*:\\s*(\"([^\"]+)\"|(\\d+))`);
      const match = raw.match(pattern);
      if (match) {
        const textValue = match.length >= 3 ? match[2] : '';
        if (typeof textValue === 'string' && textValue.trim().length > 0) {
          return textValue.trim();
        }
        const numberValue = match.length >= 4 ? match[3] : '';
        if (typeof numberValue === 'string' && numberValue.trim().length > 0) {
          return numberValue.trim();
        }
      }
    }
    return '';
  }

  private safeGet(obj: Object, key: string): Object | null | undefined {
    try {
      return Reflect.get(obj, key) as Object | null | undefined;
    } catch (_e) {
      return null;
    }
  }

  private normalizeText(raw: Object | null | undefined): string {
    if (raw === null || raw === undefined) {
      return '';
    }
    const text = typeof raw === 'string' ? raw : String(raw);
    const trimmed = text.trim();
    if (trimmed === 'undefined' || trimmed === 'null') {
      return '';
    }
    return trimmed;
  }

  private sanitizeFormId(candidate: string): string {
    const trimmed = candidate.trim();
    if (trimmed === 'undefined' || trimmed === 'null') {
      return '';
    }
    return trimmed;
  }

  private safeStringify(value: Object): string {
    try {
      const raw = JSON.stringify(value);
      return typeof raw === 'string' ? raw : '';
    } catch (_e) {
      return '';
    }
  }
}
