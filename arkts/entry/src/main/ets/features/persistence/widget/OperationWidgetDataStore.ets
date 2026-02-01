import distributedKVStore from '@ohos.data.distributedKVStore';
import type { common } from '@kit.AbilityKit';

const STORE_ID: string = 'operation_widget_data';
const STORE_BUNDLE_NAME: string = 'com.huawei.aios.agent.operation';
const KEY_FAVORITES: string = 'W2:favorites';
const KEY_UPDATED_AT_MS: string = 'W2:updated_at_ms';

export interface OperationWidgetFavoritesSnapshot {
  favorites: Array<string>;
  updatedAtMs: number;
}

export class OperationWidgetDataStore {
  private static manager: distributedKVStore.KVManager | null = null;
  private static store: distributedKVStore.SingleKVStore | null = null;
  private static storePromise: Promise<distributedKVStore.SingleKVStore> | null = null;
  private static writeChain: Promise<void> = Promise.resolve();

  static async setFavorites(ctx: common.Context, favorites: Array<string>): Promise<void> {
    await OperationWidgetDataStore.enqueueWrite(async () => {
      const store = await OperationWidgetDataStore.getStore(ctx);
      const picked: Array<string> = [];
      for (let i = 0; i < favorites.length && picked.length < 4; i += 1) {
        const name = favorites[i].trim();
        if (name.length > 0) {
          picked.push(name);
        }
      }
      await store.put(KEY_FAVORITES, JSON.stringify(picked));
      await store.put(KEY_UPDATED_AT_MS, String(Date.now()));
    });
  }

  static async getSnapshot(ctx: common.Context): Promise<OperationWidgetFavoritesSnapshot> {
    const store = await OperationWidgetDataStore.getStore(ctx);
    const raw = await OperationWidgetDataStore.safeGetString(store, KEY_FAVORITES);
    const tsRaw = await OperationWidgetDataStore.safeGetString(store, KEY_UPDATED_AT_MS);
    const updatedAtMs = OperationWidgetDataStore.parseMs(tsRaw, 0);
    if (raw.trim().length === 0) {
      return { favorites: [], updatedAtMs };
    }
    try {
      const parsed = JSON.parse(raw) as Array<Object>;
      if (!Array.isArray(parsed)) {
        return { favorites: [], updatedAtMs };
      }
      const result: Array<string> = [];
      for (let i = 0; i < parsed.length && result.length < 4; i += 1) {
        const name = OperationWidgetDataStore.normalizeText(parsed[i]);
        if (name.length > 0) {
          result.push(name);
        }
      }
      return { favorites: result, updatedAtMs };
    } catch {
      return { favorites: [], updatedAtMs };
    }
  }

  private static async getStore(ctx: common.Context): Promise<distributedKVStore.SingleKVStore> {
    if (OperationWidgetDataStore.store) {
      return OperationWidgetDataStore.store;
    }
    if (OperationWidgetDataStore.storePromise) {
      return await OperationWidgetDataStore.storePromise;
    }
    OperationWidgetDataStore.storePromise = OperationWidgetDataStore.openStore(ctx);
    try {
      const store = await OperationWidgetDataStore.storePromise;
      OperationWidgetDataStore.store = store;
      return store;
    } finally {
      OperationWidgetDataStore.storePromise = null;
    }
  }

  private static async openStore(ctx: common.Context): Promise<distributedKVStore.SingleKVStore> {
    if (!OperationWidgetDataStore.manager) {
      const config: distributedKVStore.KVManagerConfig = {
        context: ctx,
        bundleName: STORE_BUNDLE_NAME,
      };
      OperationWidgetDataStore.manager = distributedKVStore.createKVManager(config);
    }
    const options: distributedKVStore.Options = {
      createIfMissing: true,
      encrypt: false,
      backup: false,
      autoSync: false,
      kvStoreType: distributedKVStore.KVStoreType.SINGLE_VERSION,
      securityLevel: distributedKVStore.SecurityLevel.S1,
    };
    return await OperationWidgetDataStore.manager.getKVStore<distributedKVStore.SingleKVStore>(STORE_ID, options);
  }

  private static async enqueueWrite(fn: () => Promise<void>): Promise<void> {
    const chained = OperationWidgetDataStore.writeChain.then(fn).catch(() => {});
    OperationWidgetDataStore.writeChain = chained;
    await chained;
  }

  private static async safeGetString(store: distributedKVStore.SingleKVStore, key: string): Promise<string> {
    try {
      const value = await store.get(key);
      if (typeof value === 'string') {
        return value;
      }
      return String(value ?? '');
    } catch {
      return '';
    }
  }

  private static normalizeText(raw: Object | null | undefined): string {
    if (raw === null || raw === undefined) {
      return '';
    }
    const text = typeof raw === 'string' ? raw : String(raw);
    const trimmed = text.trim();
    if (trimmed.length === 0 || trimmed === 'undefined' || trimmed === 'null') {
      return '';
    }
    return trimmed;
  }

  private static parseMs(raw: string, fallback: number): number {
    const trimmed = raw.trim();
    if (trimmed.length === 0) {
      return fallback;
    }
    const parsed = parseInt(trimmed, 10);
    if (!Number.isFinite(parsed)) {
      return fallback;
    }
    return Math.max(0, parsed);
  }
}

