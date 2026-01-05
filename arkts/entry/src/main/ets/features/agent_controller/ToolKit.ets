import type { AgentAction, ActionResult, AgentSpaceRuntimeInfo } from './agentspace/AgentSpace';
import { AgentSpace } from './agentspace/AgentSpace';
import type { AppTarget } from './agentspace/VTSBackend';
import type { AppTargetDescriptor } from './types';

export class ToolKit {
  private agentSpace: AgentSpace;
  private appTargets: Map<string, AppTarget> = new Map<string, AppTarget>();

  constructor(agentSpace: AgentSpace) {
    this.agentSpace = agentSpace;
  }

  setAppTargets(targets: Array<AppTargetDescriptor>): void {
    this.appTargets.clear();
    for (let i = 0; i < targets.length; i++) {
      const entry = targets[i];
      const displayName = entry.displayName.trim();
      const bundleName = entry.bundleName.trim();
      if (displayName.length === 0 || bundleName.length === 0) {
        continue;
      }
      const abilityName = entry.abilityName.trim().length > 0 ? entry.abilityName.trim() : 'EntryAbility';
      const target: AppTarget = { bundleName, abilityName };
      this.appTargets.set(displayName, target);
    }
  }

  async ensureVts(): Promise<AgentSpaceRuntimeInfo> {
    return await this.agentSpace.ensureVts();
  }

  async getRuntimeInfo(): Promise<AgentSpaceRuntimeInfo> {
    return await this.agentSpace.getRuntimeInfo();
  }

  async launchApp(target: AppTarget): Promise<void> {
    await this.agentSpace.startApp(target);
  }

  async clickNorm(xNorm: number, yNorm: number): Promise<void> {
    const rt = await this.agentSpace.ensureVts();
    const x = this.normToPx(rt.widthPx, xNorm);
    const y = this.normToPx(rt.heightPx, yNorm);
    await this.agentSpace.clickPx(x, y);
  }

  async screenshotBase64(): Promise<string> {
    await this.agentSpace.ensureVts();
    return await this.agentSpace.screenshotBase64();
  }

  async transferToMain(): Promise<number> {
    await this.agentSpace.ensureVts();
    return await this.agentSpace.transferToMain();
  }

  async executeAction(action: AgentAction): Promise<ActionResult> {
    const name = action.name.trim().toLowerCase();
    if (name === 'launch_app') {
      const appName = action.parameters?.app ?? '';
      const target = this.resolveAppTarget(appName);
      if (!target) {
        return { status: 'FAILED', error: 'app_not_mapped' };
      }
      await this.launchApp(target);
      return { status: 'SUCCESS' };
    }
    return await this.agentSpace.executeAction(action);
  }

  async destroy(): Promise<void> {
    await this.agentSpace.destroy();
  }

  async stop(): Promise<number> {
    await this.agentSpace.ensureVts();
    const count = await this.agentSpace.transferToMain();
    await this.agentSpace.destroy();
    return count;
  }

  private normToPx(limit: number, value: number): number {
    const safe = Number.isFinite(value) ? value : 0;
    const clamped = Math.min(Math.max(safe, 0), 1000);
    const max = Math.max(1, limit);
    const px = Math.round((clamped / 1000) * (max - 1));
    return px < 0 ? 0 : px;
  }

  private resolveAppTarget(rawName: string): AppTarget | null {
    const name = rawName.trim();
    if (name.length === 0) {
      return null;
    }
    const direct = this.parseDirectTarget(name);
    if (direct) {
      return direct;
    }
    const mapped = this.appTargets.get(name);
    if (mapped) {
      return mapped;
    }
    const extracted = this.extractBundleNameSuffix(name);
    if (extracted.length > 0) {
      return { bundleName: extracted, abilityName: 'EntryAbility' };
    }
    return null;
  }

  private parseDirectTarget(name: string): AppTarget | null {
    const slashIndex = name.indexOf('/');
    if (slashIndex > 0 && slashIndex < name.length - 1) {
      const bundleName = name.slice(0, slashIndex).trim();
      const abilityName = name.slice(slashIndex + 1).trim();
      if (bundleName.length === 0 || abilityName.length === 0) {
        return null;
      }
      return { bundleName, abilityName };
    }
    if (this.looksLikeBundleName(name)) {
      return { bundleName: name, abilityName: 'EntryAbility' };
    }
    return null;
  }

  private looksLikeBundleName(name: string): boolean {
    const normalized = name.trim();
    if (normalized.length === 0) {
      return false;
    }
    if (normalized.startsWith('com.') || normalized.startsWith('ohos.') || normalized.startsWith('cn.')) {
      return true;
    }
    return normalized.indexOf('.') >= 0;
  }

  private extractBundleNameSuffix(input: string): string {
    if (!input.endsWith(')')) {
      return '';
    }
    const openIndex = input.lastIndexOf('(');
    if (openIndex < 0) {
      return '';
    }
    return input.slice(openIndex + 1, input.length - 1).trim();
  }
}
