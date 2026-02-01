import preferences from '@ohos.data.preferences';
import type { common } from '@kit.AbilityKit';

const PREFS_NAME: string = 'operation_widget_data';
const KEY_FAVORITES: string = 'favorites';
const KEY_UPDATED_AT_MS: string = 'updated_at_ms';

export interface OperationWidgetFavoritesSnapshot {
  favorites: Array<string>;
  updatedAtMs: number;
}

export class OperationWidgetDataStore {
  static async setFavorites(ctx: common.Context, favorites: Array<string>): Promise<void> {
    const prefs = await preferences.getPreferences(ctx, PREFS_NAME);
    const trimmed: Array<string> = [];
    for (let i = 0; i < favorites.length && trimmed.length < 4; i += 1) {
      const name = favorites[i].trim();
      if (name.length > 0) {
        trimmed.push(name);
      }
    }
    await prefs.put(KEY_FAVORITES, JSON.stringify(trimmed));
    await prefs.put(KEY_UPDATED_AT_MS, Date.now());
    await prefs.flush();
  }

  static async getFavorites(ctx: common.Context): Promise<Array<string>> {
    const snapshot = await OperationWidgetDataStore.getSnapshot(ctx);
    return snapshot.favorites;
  }

  static async getSnapshot(ctx: common.Context): Promise<OperationWidgetFavoritesSnapshot> {
    const prefs = await preferences.getPreferences(ctx, PREFS_NAME);
    const raw = await prefs.get(KEY_FAVORITES, '[]');
    const tsRaw = await prefs.get(KEY_UPDATED_AT_MS, 0);
    const updatedAtMs = OperationWidgetDataStore.parseMs(tsRaw, 0);
    const text = OperationWidgetDataStore.normalizeText(raw, '[]');
    if (!text.startsWith('[')) {
      return { favorites: [], updatedAtMs };
    }
    try {
      const parsed = JSON.parse(text) as Array<Object>;
      if (!Array.isArray(parsed)) {
        return { favorites: [], updatedAtMs };
      }
      const result: Array<string> = [];
      for (let i = 0; i < parsed.length && result.length < 4; i += 1) {
        const value = parsed[i];
        const name = OperationWidgetDataStore.normalizeText(value, '');
        if (name.length > 0) {
          result.push(name);
        }
      }
      return { favorites: result, updatedAtMs };
    } catch {
      return { favorites: [], updatedAtMs };
    }
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
}
