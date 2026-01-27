import type { DeviceState } from '../types';
import { ToolKit } from '../toolkit/ToolKit';
import { ForegroundAppState } from './ForegroundAppState';
import type { GpsInfo } from './LocationState';
import { LocationState } from './LocationState';
import { AccessibilityTreeClient } from './accessibility/AccessibilityTreeClient';
import { AccessibilityTreeFormatter } from './accessibility/AccessibilityTreeFormatter';
import type { AccessibilityTreeRequestOptions } from './accessibility/AccessibilityTreeProtocol';

export class StateCollector {
  private toolKit: ToolKit;

  constructor(toolKit: ToolKit) {
    this.toolKit = toolKit;
  }

  async screenshotBase64(): Promise<string> {
    return await this.toolKit.screenshotBase64();
  }

  async getForegroundAppBundleName(): Promise<string> {
    return await ForegroundAppState.getForegroundAppBundleName();
  }

  async getCurrentLocation(): Promise<GpsInfo | null> {
    return await LocationState.getCurrentLocation();
  }

  async buildState(
    screenshotBase64: string,
    installedApps: Array<string>,
    includeInstalledApps: boolean,
    userNote: string
  ): Promise<DeviceState> {
    const state: DeviceState = { screenshot: screenshotBase64 };
    const foregroundBundle = (await this.getForegroundAppBundleName()).trim();
    if (foregroundBundle.length > 0) {
      state.system_foreground_app_bundle = foregroundBundle;
    }
    const agentSpaceForeground = this.toolKit.getCurrentAppBundleFromAgentSpace().trim();
    if (agentSpaceForeground.length > 0) {
      const displayName = this.toolKit.resolveAppDisplayNameByBundleName(agentSpaceForeground).trim();
      state.agentspace_foreground_app_bundle =
        displayName.length > 0 ? `${displayName} (${agentSpaceForeground})` : agentSpaceForeground;
    }
    const note = userNote.trim();
    if (note.length > 0) {
      state.user_note = note;
    }
    if (includeInstalledApps && installedApps.length > 0) {
      state.installed_apps = installedApps.slice();
    }
    return state;
  }

  async dumpAccessibilityTreeForAgentSpace(): Promise<string> {
    try {
      let runtime = await this.toolKit.getRuntimeInfo();
      if (runtime.screenId < 0) {
        runtime = await this.toolKit.ensureVts();
      }
      const displayId =
        typeof runtime.displayId === 'number' && Number.isFinite(runtime.displayId) && runtime.displayId >= 0
          ? runtime.displayId
          : undefined;
      const targetBundle = this.toolKit.getCurrentAppBundleFromAgentSpace().trim();
      if (targetBundle.length === 0) {
        return 'ERR:agentspace_bundle_missing';
      }
      const request: AccessibilityTreeRequestOptions = {
        displayId,
        bundleName: targetBundle,
        maxDepth: 10,
        maxNodes: 120
      };
      const response = await AccessibilityTreeClient.requestTreeJson(request);
      return response;
    } catch (e) {
      return `ERR:${String(e)}`;
    }
  }

  async dumpAccessibilityWindowsForAgentSpace(): Promise<string> {
    try {
      let runtime = await this.toolKit.getRuntimeInfo();
      if (runtime.screenId < 0) {
        runtime = await this.toolKit.ensureVts();
      }
      const displayId =
        typeof runtime.displayId === 'number' && Number.isFinite(runtime.displayId) && runtime.displayId >= 0
          ? runtime.displayId
          : undefined;
      const request: AccessibilityTreeRequestOptions = {
        displayId,
        windowsOnly: true
      };
      const response = await AccessibilityTreeClient.requestWindowsJson(request);
      return response;
    } catch (e) {
      return `ERR:${String(e)}`;
    }
  }

  async dumpAccessibilityTreeListForAgentSpace(): Promise<string> {
    const raw = await this.dumpAccessibilityTreeForAgentSpace();
    if (raw.startsWith('ERR:') || raw.startsWith('ERR_EXCEPTION:')) {
      return raw;
    }
    const listJson = AccessibilityTreeFormatter.toListJson(raw);
    if (listJson.startsWith('ERR:')) {
      return listJson;
    }
    return listJson;
  }

  async dumpAccessibilityNodeAttributesForAgentSpace(): Promise<string> {
    const raw = await this.dumpAccessibilityTreeForAgentSpace();
    if (raw.startsWith('ERR:') || raw.startsWith('ERR_EXCEPTION:')) {
      return raw;
    }
    const lines = AccessibilityTreeFormatter.toDebugLines(raw);
    if (!lines) {
      return 'ERR:accessibility_tree_parse_failed';
    }
    return lines.join('\n');
  }
}
