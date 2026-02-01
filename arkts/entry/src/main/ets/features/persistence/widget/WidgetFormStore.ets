import type { common } from '@kit.AbilityKit';
import distributedKVStore from '@ohos.data.distributedKVStore';
import preferences from '@ohos.data.preferences';

export interface WidgetFormRecord {
  formId: string;
  widgetName: string;
  dimension: string;
  createdAtMs: number;
  updatedAtMs: number;
}

const STORE_ID: string = 'widget_form_store';
const STORE_BUNDLE_NAME: string = 'com.huawei.aios.agent.operation';
const KEY_RECORDS: string = 'W1:records';
const PREFS_NAME: string = 'widget_form_store';
const PREFS_KEY_RECORDS: string = 'records';
const MAX_RECORDS: number = 32;

export class WidgetFormStore {
  private static manager: distributedKVStore.KVManager | null = null;
  private static store: distributedKVStore.SingleKVStore | null = null;
  private static storePromise: Promise<distributedKVStore.SingleKVStore> | null = null;
  private static writeChain: Promise<void> = Promise.resolve();

  static async list(ctx: common.Context): Promise<Array<WidgetFormRecord>> {
    const store = await WidgetFormStore.getStore(ctx);
    const raw = await store.get(KEY_RECORDS);
    const records = WidgetFormStore.parseRecords(raw);
    if (records.length > 0) {
      return records;
    }
    // Backward-compatible fallback (older versions used preferences).
    const legacy = await WidgetFormStore.listFromPrefsBestEffort(ctx);
    if (legacy.length > 0) {
      void WidgetFormStore.migrateFromPrefsBestEffort(ctx, legacy);
      return legacy;
    }
    return [];
  }

  static async upsert(ctx: common.Context, record: WidgetFormRecord): Promise<void> {
    await WidgetFormStore.enqueueWrite(async () => {
      const store = await WidgetFormStore.getStore(ctx);
      const raw = await store.get(KEY_RECORDS);
      const current = WidgetFormStore.parseRecords(raw);
      const next: Array<WidgetFormRecord> = WidgetFormStore.upsertInto(current, record);
      await store.put(KEY_RECORDS, JSON.stringify(next));
      await WidgetFormStore.writePrefsBestEffort(ctx, next);
    });
  }

  static async remove(ctx: common.Context, formId: string): Promise<void> {
    const trimmed = formId.trim();
    if (trimmed.length === 0 || trimmed === 'undefined' || trimmed === 'null') {
      return;
    }
    await WidgetFormStore.enqueueWrite(async () => {
      const store = await WidgetFormStore.getStore(ctx);
      const raw = await store.get(KEY_RECORDS);
      const current = WidgetFormStore.parseRecords(raw);
      const next: Array<WidgetFormRecord> = [];
      for (let i = 0; i < current.length; i += 1) {
        const item = current[i];
        if (item.formId !== trimmed) {
          next.push(item);
        }
      }
      await store.put(KEY_RECORDS, JSON.stringify(next));
      await WidgetFormStore.writePrefsBestEffort(ctx, next);
    });
  }

  static async clear(ctx: common.Context): Promise<void> {
    await WidgetFormStore.enqueueWrite(async () => {
      const store = await WidgetFormStore.getStore(ctx);
      await store.put(KEY_RECORDS, '[]');
      await WidgetFormStore.writePrefsBestEffort(ctx, []);
    });
  }

  private static async getStore(ctx: common.Context): Promise<distributedKVStore.SingleKVStore> {
    if (WidgetFormStore.store) {
      return WidgetFormStore.store;
    }
    if (WidgetFormStore.storePromise) {
      return await WidgetFormStore.storePromise;
    }
    WidgetFormStore.storePromise = WidgetFormStore.openStore(ctx);
    try {
      const store = await WidgetFormStore.storePromise;
      WidgetFormStore.store = store;
      return store;
    } finally {
      WidgetFormStore.storePromise = null;
    }
  }

  private static async openStore(ctx: common.Context): Promise<distributedKVStore.SingleKVStore> {
    if (!WidgetFormStore.manager) {
      WidgetFormStore.manager = distributedKVStore.createKVManager({
        context: ctx,
        bundleName: STORE_BUNDLE_NAME,
      });
    }
    const options: distributedKVStore.Options = {
      createIfMissing: true,
      encrypt: false,
      backup: false,
      autoSync: false,
      kvStoreType: distributedKVStore.KVStoreType.SINGLE_VERSION,
      securityLevel: distributedKVStore.SecurityLevel.S1,
    };
    return await WidgetFormStore.manager.getKVStore(STORE_ID, options) as distributedKVStore.SingleKVStore;
  }

  private static async enqueueWrite(fn: () => Promise<void>): Promise<void> {
    WidgetFormStore.writeChain = WidgetFormStore.writeChain
      .then(fn)
      .catch((_e) => {
        // swallow errors to avoid breaking chain
      });
    await WidgetFormStore.writeChain;
  }

  private static async listFromPrefsBestEffort(ctx: common.Context): Promise<Array<WidgetFormRecord>> {
    try {
      const prefs = await preferences.getPreferences(ctx, PREFS_NAME);
      const raw = await prefs.get(PREFS_KEY_RECORDS, '[]');
      return WidgetFormStore.parseRecords(raw);
    } catch (_e) {
      return [];
    }
  }

  private static async writePrefsBestEffort(ctx: common.Context, records: Array<WidgetFormRecord>): Promise<void> {
    try {
      const prefs = await preferences.getPreferences(ctx, PREFS_NAME);
      await prefs.put(PREFS_KEY_RECORDS, JSON.stringify(records));
      await prefs.flush();
    } catch (_e) {
      // ignore
    }
  }

  private static async migrateFromPrefsBestEffort(ctx: common.Context, legacy: Array<WidgetFormRecord>): Promise<void> {
    try {
      const store = await WidgetFormStore.getStore(ctx);
      await store.put(KEY_RECORDS, JSON.stringify(legacy));
    } catch (_e) {
      // ignore
    }
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
