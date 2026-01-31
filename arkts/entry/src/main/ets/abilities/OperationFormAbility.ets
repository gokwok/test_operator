import type { Want } from '@kit.AbilityKit';
import { FormExtensionAbility, formBindingData, formProvider } from '@kit.FormKit';
import { hilog } from '@kit.PerformanceAnalysisKit';

import { WidgetFormStore, WidgetFormRecord } from '../features/persistence/widget/WidgetFormStore';

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

export default class OperationFormAbility extends FormExtensionAbility {
  onAddForm(want: Want): formBindingData.FormBindingData {
    hilog.info(HILOG_DOMAIN, HILOG_TAG, 'onAddForm want=%{public}s', JSON.stringify(want));
    void this.trackFormAdded(want);
    return formBindingData.createFormBindingData(this.buildDefaultData());
  }

  onUpdateForm(formId: string): void {
    hilog.info(HILOG_DOMAIN, HILOG_TAG, 'onUpdateForm formId=%{public}s', formId);
    const data = this.buildDefaultData();
    formProvider.updateForm(formId, formBindingData.createFormBindingData(data));
    void this.trackFormUpdated(formId);
  }

  onRemoveForm(formId: string): void {
    hilog.info(HILOG_DOMAIN, HILOG_TAG, 'onRemoveForm formId=%{public}s', formId);
    void this.trackFormRemoved(formId);
  }

  private buildDefaultData(): OperationWidgetFormData {
    const data: OperationWidgetFormData = {
      title: '操作智能体',
      shortcut1: '快捷 1',
      shortcut2: '快捷 2',
      shortcut3: '快捷 3',
      shortcut4: '快捷 4',
    };
    return data;
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
      const trimmed = formId.trim();
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
      'ohos.extra.param.key.form_id',
      'ohos.extra.param.key.formId',
      'ohos.extra.param.key.FORM_ID',
      'ohos.extra.form_id',
      'ohos.extra.formId',
    ]);
    if (formId.length > 0) {
      return formId;
    }
    const raw = this.safeStringify(want as Object);
    if (raw.length === 0) {
      return '';
    }
    return this.extractFirstJsonField(raw, [
      'formId',
      'form_id',
      'ohos.extra.param.key.form_id',
      'ohos.extra.form_id',
    ]);
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
      const pattern = new RegExp(`\"${escaped}\"\\s*:\\s*\"([^\"]+)\"`);
      const match = raw.match(pattern);
      if (match && match.length >= 2) {
        const value = match[1].trim();
        if (value.length > 0) {
          return value;
        }
      }
    }
    return '';
  }

  private safeGet(obj: Object, key: string): Object {
    try {
      return Reflect.get(obj, key) as Object;
    } catch (_e) {
      return '';
    }
  }

  private normalizeText(raw: Object): string {
    const text = typeof raw === 'string' ? raw : String(raw);
    return text.trim();
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
