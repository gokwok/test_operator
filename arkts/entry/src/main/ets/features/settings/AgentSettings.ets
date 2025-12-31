import preferences from '@ohos.data.preferences';
import type { common } from '@kit.AbilityKit';

export interface AgentSettingsData {
  cloudEndpoint: string;
  waitMs: number;
  liveViewEnabled: boolean;
}

export interface AgentSettingsPatch {
  cloudEndpoint?: string;
  waitMs?: number;
  liveViewEnabled?: boolean;
}

const PREFS_NAME: string = 'agent_settings';
const KEY_CLOUD_ENDPOINT: string = 'cloud_endpoint';
const KEY_WAIT_MS: string = 'wait_ms';
const KEY_LIVE_VIEW: string = 'live_view_enabled';

const DEFAULT_CLOUD_ENDPOINT: string = 'http://114.55.173.20:8005';
const DEFAULT_WAIT_MS: number = 500;
const DEFAULT_LIVE_VIEW_ENABLED: boolean = true;

export class AgentSettings {
  static defaults(): AgentSettingsData {
    return {
      cloudEndpoint: DEFAULT_CLOUD_ENDPOINT,
      waitMs: DEFAULT_WAIT_MS,
      liveViewEnabled: DEFAULT_LIVE_VIEW_ENABLED,
    };
  }

  static async load(ctx: common.Context): Promise<AgentSettingsData> {
    const prefs = await preferences.getPreferences(ctx, PREFS_NAME);
    const endpointRaw = await prefs.get(KEY_CLOUD_ENDPOINT, DEFAULT_CLOUD_ENDPOINT);
    const waitRaw = await prefs.get(KEY_WAIT_MS, DEFAULT_WAIT_MS);
    const liveViewRaw = await prefs.get(KEY_LIVE_VIEW, DEFAULT_LIVE_VIEW_ENABLED);

    const endpoint = typeof endpointRaw === 'string' ? endpointRaw : String(endpointRaw);
    const waitMs = AgentSettings.parseWaitMs(waitRaw, DEFAULT_WAIT_MS);
    const liveViewEnabled = AgentSettings.parseBoolean(liveViewRaw, DEFAULT_LIVE_VIEW_ENABLED);

    const normalizedEndpoint = endpoint.trim().length > 0 ? endpoint.trim() : DEFAULT_CLOUD_ENDPOINT;
    return {
      cloudEndpoint: normalizedEndpoint,
      waitMs,
      liveViewEnabled,
    };
  }

  static async save(ctx: common.Context, data: AgentSettingsData): Promise<void> {
    const prefs = await preferences.getPreferences(ctx, PREFS_NAME);
    await prefs.put(KEY_CLOUD_ENDPOINT, data.cloudEndpoint);
    await prefs.put(KEY_WAIT_MS, data.waitMs);
    await prefs.put(KEY_LIVE_VIEW, data.liveViewEnabled);
    await prefs.flush();
  }

  static async update(ctx: common.Context, patch: AgentSettingsPatch): Promise<AgentSettingsData> {
    const current = await AgentSettings.load(ctx);
    const next: AgentSettingsData = {
      cloudEndpoint: patch.cloudEndpoint ?? current.cloudEndpoint,
      waitMs: patch.waitMs ?? current.waitMs,
      liveViewEnabled: patch.liveViewEnabled ?? current.liveViewEnabled,
    };
    await AgentSettings.save(ctx, next);
    return next;
  }

  private static parseWaitMs(raw: Object, fallback: number): number {
    if (typeof raw === 'number' && Number.isFinite(raw)) {
      return Math.max(0, Math.floor(raw));
    }
    const parsed = parseInt(String(raw), 10);
    if (!Number.isFinite(parsed)) {
      return fallback;
    }
    return Math.max(0, parsed);
  }

  private static parseBoolean(raw: Object, fallback: boolean): boolean {
    if (typeof raw === 'boolean') {
      return raw;
    }
    const text = String(raw).trim().toLowerCase();
    if (text === 'true') {
      return true;
    }
    if (text === 'false') {
      return false;
    }
    return fallback;
  }
}
