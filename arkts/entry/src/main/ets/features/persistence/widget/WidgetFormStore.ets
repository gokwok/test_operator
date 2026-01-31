import preferences from '@ohos.data.preferences';
import type { common } from '@kit.AbilityKit';

export interface WidgetFormRecord {
  formId: string;
  widgetName: string;
  dimension: string;
  createdAtMs: number;
  updatedAtMs: number;
}

const PREFS_NAME: string = 'widget_form_store';
const KEY_RECORDS: string = 'records';
const MAX_RECORDS: number = 32;

export class WidgetFormStore {
  static async list(ctx: common.Context): Promise<Array<WidgetFormRecord>> {
    const prefs = await preferences.getPreferences(ctx, PREFS_NAME);
    const raw = await prefs.get(KEY_RECORDS, '[]');
    return WidgetFormStore.parseRecords(raw);
  }

  static async upsert(ctx: common.Context, record: WidgetFormRecord): Promise<void> {
    const prefs = await preferences.getPreferences(ctx, PREFS_NAME);
    const raw = await prefs.get(KEY_RECORDS, '[]');
    const current = WidgetFormStore.parseRecords(raw);
    const next: Array<WidgetFormRecord> = WidgetFormStore.upsertInto(current, record);
    await prefs.put(KEY_RECORDS, JSON.stringify(next));
    await prefs.flush();
  }

  static async remove(ctx: common.Context, formId: string): Promise<void> {
    const trimmed = formId.trim();
    if (trimmed.length === 0 || trimmed === 'undefined' || trimmed === 'null') {
      return;
    }
    const prefs = await preferences.getPreferences(ctx, PREFS_NAME);
    const raw = await prefs.get(KEY_RECORDS, '[]');
    const current = WidgetFormStore.parseRecords(raw);
    const next: Array<WidgetFormRecord> = [];
    for (let i = 0; i < current.length; i += 1) {
      const item = current[i];
      if (item.formId !== trimmed) {
        next.push(item);
      }
    }
    await prefs.put(KEY_RECORDS, JSON.stringify(next));
    await prefs.flush();
  }

  static async clear(ctx: common.Context): Promise<void> {
    const prefs = await preferences.getPreferences(ctx, PREFS_NAME);
    await prefs.put(KEY_RECORDS, '[]');
    await prefs.flush();
  }

  private static upsertInto(current: Array<WidgetFormRecord>, record: WidgetFormRecord): Array<WidgetFormRecord> {
    const formId = record.formId.trim();
    if (formId.length === 0 || formId === 'undefined' || formId === 'null') {
      return current;
    }
    const now = Date.now();
    const next: Array<WidgetFormRecord> = [];
    let replaced = false;
    for (let i = 0; i < current.length; i += 1) {
      const item = current[i];
      if (!replaced && item.formId === formId) {
        const merged: WidgetFormRecord = {
          formId,
          widgetName: record.widgetName.trim(),
          dimension: record.dimension.trim(),
          createdAtMs: item.createdAtMs > 0 ? item.createdAtMs : now,
          updatedAtMs: now,
        };
        next.push(merged);
        replaced = true;
      } else {
        next.push(item);
      }
    }
    if (!replaced) {
      const added: WidgetFormRecord = {
        formId,
        widgetName: record.widgetName.trim(),
        dimension: record.dimension.trim(),
        createdAtMs: now,
        updatedAtMs: now,
      };
      next.unshift(added);
    }
    if (next.length > MAX_RECORDS) {
      return next.slice(0, MAX_RECORDS);
    }
    return next;
  }

  private static parseRecords(raw: Object): Array<WidgetFormRecord> {
    const text = WidgetFormStore.normalizeText(raw, '[]');
    if (!text.startsWith('[')) {
      return [];
    }
    try {
      const parsed = JSON.parse(text) as Array<Object>;
      if (!Array.isArray(parsed)) {
        return [];
      }
      const records: Array<WidgetFormRecord> = [];
      for (let i = 0; i < parsed.length; i += 1) {
        const value = parsed[i];
        const record = WidgetFormStore.parseRecord(value);
        if (record) {
          records.push(record);
        }
      }
      return records;
    } catch (_e) {
      return [];
    }
  }

  private static parseRecord(value: Object): WidgetFormRecord | null {
    if (!value || typeof value !== 'object') {
      return null;
    }
    const formId = WidgetFormStore.normalizeText(WidgetFormStore.safeGet(value, 'formId'), '');
    if (formId.length === 0) {
      return null;
    }
    const widgetName = WidgetFormStore.normalizeText(WidgetFormStore.safeGet(value, 'widgetName'), '');
    const dimension = WidgetFormStore.normalizeText(WidgetFormStore.safeGet(value, 'dimension'), '');
    const createdAtMs = WidgetFormStore.parseMs(WidgetFormStore.safeGet(value, 'createdAtMs'), 0);
    const updatedAtMs = WidgetFormStore.parseMs(WidgetFormStore.safeGet(value, 'updatedAtMs'), 0);
    const record: WidgetFormRecord = {
      formId,
      widgetName,
      dimension,
      createdAtMs,
      updatedAtMs,
    };
    return record;
  }

  private static safeGet(obj: Object, key: string): Object | null | undefined {
    try {
      return Reflect.get(obj, key) as Object | null | undefined;
    } catch (_e) {
      return null;
    }
  }

  private static parseMs(raw: Object | null | undefined, fallback: number): number {
    if (raw === null || raw === undefined) {
      return fallback;
    }
    if (typeof raw === 'number' && Number.isFinite(raw)) {
      return Math.max(0, Math.floor(raw));
    }
    const parsed = parseInt(String(raw), 10);
    if (!Number.isFinite(parsed)) {
      return fallback;
    }
    return Math.max(0, parsed);
  }

  private static normalizeText(raw: Object | null | undefined, fallback: string): string {
    if (raw === null || raw === undefined) {
      return fallback;
    }
    const text = typeof raw === 'string' ? raw : String(raw);
    const trimmed = text.trim();
    if (trimmed.length === 0 || trimmed === 'undefined' || trimmed === 'null') {
      return fallback;
    }
    return trimmed;
  }
}
