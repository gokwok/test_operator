import type { DeviceState } from '../types';
import { ToolKit } from '../toolkit/ToolKit';
import { ForegroundAppState } from './ForegroundAppState';
import type { GpsInfo } from './LocationState';
import { LocationState } from './LocationState';

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
}
