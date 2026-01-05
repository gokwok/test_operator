import type { DeviceState } from '../types';
import { ToolKit } from '../ToolKit';

export class StateCollector {
  private toolKit: ToolKit;

  constructor(toolKit: ToolKit) {
    this.toolKit = toolKit;
  }

  async screenshotBase64(): Promise<string> {
    return await this.toolKit.screenshotBase64();
  }

  buildState(
    screenshotBase64: string,
    installedApps: Array<string>,
    includeInstalledApps: boolean,
    userNote: string
  ): DeviceState {
    const state: DeviceState = { screenshot: screenshotBase64 };
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
