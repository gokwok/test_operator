import type { AgentSpaceRuntimeInfo } from '../agentspace/AgentSpace';
import { AgentSpace } from '../agentspace/AgentSpace';
import type { AppTarget } from '../agentspace/VTSBackend';

export class DeviceExecutor {
  private agentSpace: AgentSpace;

  constructor(agentSpace: AgentSpace) {
    this.agentSpace = agentSpace;
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
}
