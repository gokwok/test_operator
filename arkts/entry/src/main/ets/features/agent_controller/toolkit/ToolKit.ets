import type { AgentAction, ActionResult, AgentSpaceRuntimeInfo } from '../agentspace/AgentSpace';
import { AgentSpace } from '../agentspace/AgentSpace';
import type { AppTarget } from '../agentspace/VTSBackend';
import type { AppTargetDescriptor } from '../types';
import { AppTargetResolver } from './AppTargetResolver';
import { normToPx } from './CoordinateUtils';

export class ToolKit {
  private agentSpace: AgentSpace;
  private targetResolver: AppTargetResolver = new AppTargetResolver();

  constructor(agentSpace: AgentSpace) {
    this.agentSpace = agentSpace;
  }

  setAppTargets(targets: Array<AppTargetDescriptor>): void {
    this.targetResolver.setTargets(targets);
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
    const x = normToPx(rt.widthPx, xNorm);
    const y = normToPx(rt.heightPx, yNorm);
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
      const target = this.targetResolver.resolve(appName);
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
}
