import type { AppTarget } from '../agentspace/VTSBackend';
import type { AppTargetDescriptor } from '../types';

export class AppTargetResolver {
  private appTargets: Map<string, AppTarget> = new Map<string, AppTarget>();

  setTargets(targets: Array<AppTargetDescriptor>): void {
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

  resolve(rawName: string): AppTarget | null {
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
