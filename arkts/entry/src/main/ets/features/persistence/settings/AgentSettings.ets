import preferences from '@ohos.data.preferences';
import type { common } from '@kit.AbilityKit';

export interface AgentSettingsData {
  cloudEndpoint: string;
  a2aBaseUrl: string;
  mcpWsUrl: string;
  deviceId: string;
  contextId: string;
  waitMs: number;
  liveViewEnabled: boolean;
  historyEnabled: boolean;
  historyMaxTasks: number;
}

export interface AgentSettingsPatch {
  cloudEndpoint?: string;
  a2aBaseUrl?: string;
  mcpWsUrl?: string;
  deviceId?: string;
  contextId?: string;
  waitMs?: number;
  liveViewEnabled?: boolean;
  historyEnabled?: boolean;
  historyMaxTasks?: number;
}

const PREFS_NAME: string = 'agent_settings';
const KEY_CLOUD_ENDPOINT: string = 'cloud_endpoint';
const KEY_A2A_BASE_URL: string = 'a2a_base_url';
const KEY_MCP_WS_URL: string = 'mcp_ws_url';
const KEY_DEVICE_ID: string = 'device_id';
const KEY_CONTEXT_ID: string = 'context_id';
const KEY_WAIT_MS: string = 'wait_ms';
const KEY_LIVE_VIEW: string = 'live_view_enabled';
const KEY_HISTORY_ENABLED: string = 'history_enabled';
const KEY_HISTORY_MAX_TASKS: string = 'history_max_tasks';

const DEFAULT_CLOUD_ENDPOINT: string = 'http://114.55.173.20:8005';
const DEFAULT_A2A_BASE_URL: string = 'http://127.0.0.1:8080';
const DEFAULT_MCP_WS_URL: string = 'ws://127.0.0.1:7001/ws';
const DEFAULT_DEVICE_ID: string = 'device-1';
const DEFAULT_CONTEXT_ID: string = 'device-1';
const DEFAULT_WAIT_MS: number = 1000;
const DEFAULT_LIVE_VIEW_ENABLED: boolean = true;
const DEFAULT_HISTORY_ENABLED: boolean = true;
const DEFAULT_HISTORY_MAX_TASKS: number = 50;

export class AgentSettings {
  static defaults(): AgentSettingsData {
    return {
      cloudEndpoint: DEFAULT_CLOUD_ENDPOINT,
      a2aBaseUrl: DEFAULT_A2A_BASE_URL,
      mcpWsUrl: DEFAULT_MCP_WS_URL,
      deviceId: DEFAULT_DEVICE_ID,
      contextId: DEFAULT_CONTEXT_ID,
      waitMs: DEFAULT_WAIT_MS,
      liveViewEnabled: DEFAULT_LIVE_VIEW_ENABLED,
      historyEnabled: DEFAULT_HISTORY_ENABLED,
      historyMaxTasks: DEFAULT_HISTORY_MAX_TASKS,
    };
  }

  static async load(ctx: common.Context): Promise<AgentSettingsData> {
    const prefs = await preferences.getPreferences(ctx, PREFS_NAME);
    const endpointRaw = await prefs.get(KEY_CLOUD_ENDPOINT, DEFAULT_CLOUD_ENDPOINT);
    const a2aRaw = await prefs.get(KEY_A2A_BASE_URL, DEFAULT_A2A_BASE_URL);
    const mcpWsRaw = await prefs.get(KEY_MCP_WS_URL, DEFAULT_MCP_WS_URL);
    const deviceIdRaw = await prefs.get(KEY_DEVICE_ID, DEFAULT_DEVICE_ID);
    const contextIdRaw = await prefs.get(KEY_CONTEXT_ID, DEFAULT_CONTEXT_ID);
    const waitRaw = await prefs.get(KEY_WAIT_MS, DEFAULT_WAIT_MS);
    const liveViewRaw = await prefs.get(KEY_LIVE_VIEW, DEFAULT_LIVE_VIEW_ENABLED);
    const historyEnabledRaw = await prefs.get(KEY_HISTORY_ENABLED, DEFAULT_HISTORY_ENABLED);
    const historyMaxTasksRaw = await prefs.get(KEY_HISTORY_MAX_TASKS, DEFAULT_HISTORY_MAX_TASKS);

    const endpoint = AgentSettings.normalizeText(endpointRaw, DEFAULT_CLOUD_ENDPOINT);
    const a2aBaseUrl = AgentSettings.normalizeText(a2aRaw, DEFAULT_A2A_BASE_URL);
    const mcpWsUrl = AgentSettings.normalizeText(mcpWsRaw, DEFAULT_MCP_WS_URL);
    const deviceId = AgentSettings.normalizeText(deviceIdRaw, DEFAULT_DEVICE_ID);
    const rawContextId = AgentSettings.normalizeText(contextIdRaw, '');
    const waitMs = AgentSettings.parseWaitMs(waitRaw, DEFAULT_WAIT_MS);
    const liveViewEnabled = AgentSettings.parseBoolean(liveViewRaw, DEFAULT_LIVE_VIEW_ENABLED);
    const historyEnabled = AgentSettings.parseBoolean(historyEnabledRaw, DEFAULT_HISTORY_ENABLED);
    const historyMaxTasks = AgentSettings.parseCount(historyMaxTasksRaw, DEFAULT_HISTORY_MAX_TASKS);

    const normalizedEndpoint = endpoint.trim().length > 0 ? endpoint.trim() : DEFAULT_CLOUD_ENDPOINT;
    const normalizedContextId = rawContextId.length > 0 ? rawContextId : deviceId;
    return {
      cloudEndpoint: normalizedEndpoint,
      a2aBaseUrl,
      mcpWsUrl,
      deviceId,
      contextId: normalizedContextId,
      waitMs,
      liveViewEnabled,
      historyEnabled,
      historyMaxTasks,
    };
  }

  static async save(ctx: common.Context, data: AgentSettingsData): Promise<void> {
    const prefs = await preferences.getPreferences(ctx, PREFS_NAME);
    await prefs.put(KEY_CLOUD_ENDPOINT, data.cloudEndpoint);
    await prefs.put(KEY_A2A_BASE_URL, data.a2aBaseUrl);
    await prefs.put(KEY_MCP_WS_URL, data.mcpWsUrl);
    await prefs.put(KEY_DEVICE_ID, data.deviceId);
    await prefs.put(KEY_CONTEXT_ID, data.contextId);
    await prefs.put(KEY_WAIT_MS, data.waitMs);
    await prefs.put(KEY_LIVE_VIEW, data.liveViewEnabled);
    await prefs.put(KEY_HISTORY_ENABLED, data.historyEnabled);
    await prefs.put(KEY_HISTORY_MAX_TASKS, data.historyMaxTasks);
    await prefs.flush();
  }

  static async update(ctx: common.Context, patch: AgentSettingsPatch): Promise<AgentSettingsData> {
    const current = await AgentSettings.load(ctx);
    const next: AgentSettingsData = {
      cloudEndpoint: patch.cloudEndpoint ?? current.cloudEndpoint,
      a2aBaseUrl: patch.a2aBaseUrl ?? current.a2aBaseUrl,
      mcpWsUrl: patch.mcpWsUrl ?? current.mcpWsUrl,
      deviceId: patch.deviceId ?? current.deviceId,
      contextId: patch.contextId ?? current.contextId,
      waitMs: patch.waitMs ?? current.waitMs,
      liveViewEnabled: patch.liveViewEnabled ?? current.liveViewEnabled,
      historyEnabled: patch.historyEnabled ?? current.historyEnabled,
      historyMaxTasks: patch.historyMaxTasks ?? current.historyMaxTasks,
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

  private static parseCount(raw: Object, fallback: number): number {
    if (typeof raw === 'number' && Number.isFinite(raw)) {
      return Math.max(0, Math.floor(raw));
    }
    const parsed = parseInt(String(raw), 10);
    if (!Number.isFinite(parsed)) {
      return fallback;
    }
    return Math.max(0, parsed);
  }

  private static normalizeText(raw: Object, fallback: string): string {
    const text = typeof raw === 'string' ? raw : String(raw);
    const trimmed = text.trim();
    return trimmed.length > 0 ? trimmed : fallback;
  }
}
