import type { AgentAction, ActionResult, AgentSpaceRuntimeInfo } from '../agentspace/AgentSpace';
import { ToolKit } from '../toolkit/ToolKit';
import type { A11yTreeListItem } from '../state_collector/a11y_tree/VisiblePageContentA11yTree';
import { VisiblePageContentA11yTree } from '../state_collector/a11y_tree/VisiblePageContentA11yTree';

export interface UiTreeExpectedTarget {
  text: string;
  index: number;
}

export interface ActionExpectedResult {
  uiTarget?: UiTreeExpectedTarget;
}

export interface ActionSequenceStep {
  action: AgentAction;
  description: string;
  expected?: ActionExpectedResult;
}

export interface ActionExecutionOptions {
  windowId: number;
  defaultWaitMs: number;
  pollIntervalMs: number;
  expectTimeoutMs: number;
}

export interface ActionExecutionOptionsPatch {
  windowId?: number;
  defaultWaitMs?: number;
  pollIntervalMs?: number;
  expectTimeoutMs?: number;
}

export interface ActionExecutionObserver {
  onStepStart?: (step: ActionSequenceStep, position: number) => void | Promise<void>;
  onStepComplete?: (step: ActionSequenceStep, position: number, result: ActionResult) => void | Promise<void>;
  onWaitExpectedStart?: (step: ActionSequenceStep, position: number, expected: UiTreeExpectedTarget) => void | Promise<void>;
  onWaitExpectedDone?: (step: ActionSequenceStep, position: number, matched: boolean, elapsedMs: number) => void | Promise<void>;
}

const DEFAULT_WINDOW_ID: number = 0;
const DEFAULT_WAIT_MS: number = 1000;
const DEFAULT_POLL_INTERVAL_MS: number = 200;
const DEFAULT_EXPECT_TIMEOUT_MS: number = 5000;

export class ActionExecutionEngine {
  private readonly toolKit: ToolKit;

  constructor(toolKit: ToolKit) {
    this.toolKit = toolKit;
  }

  async run(
    steps: Array<ActionSequenceStep>,
    observer?: ActionExecutionObserver,
    options?: ActionExecutionOptionsPatch
  ): Promise<void> {
    if (!Array.isArray(steps) || steps.length === 0) {
      return;
    }
    const resolved = this.resolveOptions(options);
    const runtime = await this.toolKit.ensureVts();
    for (let i = 0; i < steps.length; i += 1) {
      const step = steps[i];
      if (observer && observer.onStepStart) {
        await observer.onStepStart(step, i);
      }
      const action = this.normalizeAction(step.action);
      const result = await this.toolKit.executeAction(action);
      if (observer && observer.onStepComplete) {
        await observer.onStepComplete(step, i, result);
      }
      if (result.status !== 'SUCCESS') {
        const err = (result.error ?? '').trim();
        throw new Error(err.length > 0 ? err : `action_failed:${action.name}`);
      }
      if (i >= steps.length - 1) {
        break;
      }
      const expected = step.expected?.uiTarget;
      if (expected && this.hasExpectedTarget(expected)) {
        if (observer && observer.onWaitExpectedStart) {
          await observer.onWaitExpectedStart(step, i, expected);
        }
        const startedAt = Date.now();
        const matched = await this.waitForExpected(runtime, resolved.windowId, expected, resolved.expectTimeoutMs, resolved.pollIntervalMs);
        const elapsedMs = Math.max(0, Date.now() - startedAt);
        if (observer && observer.onWaitExpectedDone) {
          await observer.onWaitExpectedDone(step, i, matched, elapsedMs);
        }
        continue;
      }
      await sleep(resolved.defaultWaitMs);
    }
  }

  private resolveOptions(options?: ActionExecutionOptionsPatch): ActionExecutionOptions {
    const windowIdRaw = options?.windowId;
    const defaultWaitRaw = options?.defaultWaitMs;
    const pollIntervalRaw = options?.pollIntervalMs;
    const timeoutRaw = options?.expectTimeoutMs;
    const windowId = typeof windowIdRaw === 'number' && Number.isFinite(windowIdRaw) ? Math.max(0, Math.floor(windowIdRaw)) : DEFAULT_WINDOW_ID;
    const defaultWaitMs = typeof defaultWaitRaw === 'number' && Number.isFinite(defaultWaitRaw) ? Math.max(0, Math.floor(defaultWaitRaw)) : DEFAULT_WAIT_MS;
    const pollIntervalMs = typeof pollIntervalRaw === 'number' && Number.isFinite(pollIntervalRaw) ? Math.max(50, Math.floor(pollIntervalRaw)) : DEFAULT_POLL_INTERVAL_MS;
    const expectTimeoutMs = typeof timeoutRaw === 'number' && Number.isFinite(timeoutRaw) ? Math.max(0, Math.floor(timeoutRaw)) : DEFAULT_EXPECT_TIMEOUT_MS;
    return { windowId, defaultWaitMs, pollIntervalMs, expectTimeoutMs };
  }

  private normalizeAction(action: AgentAction): AgentAction {
    const rawName = (action.name ?? '').trim().toLowerCase();
    const mapped = rawName === 'open_app' ? 'launch_app' : rawName;
    if (mapped === rawName) {
      return action;
    }
    return { name: mapped, parameters: action.parameters };
  }

  private hasExpectedTarget(expected: UiTreeExpectedTarget): boolean {
    const text = this.normalizeText(expected.text);
    const idx = this.normalizeIndex(expected.index);
    return text.length > 0 || idx > 0;
  }

  private normalizeIndex(value: number): number {
    if (typeof value !== 'number' || !Number.isFinite(value)) {
      return 0;
    }
    return Math.max(0, Math.floor(value));
  }

  private normalizeText(value: string): string {
    const raw = value.trim();
    if (raw.length === 0) {
      return '';
    }
    let out = '';
    let lastSpace = false;
    for (let i = 0; i < raw.length; i += 1) {
      const ch = raw.charAt(i);
      const isSpace = ch === ' ' || ch === '\n' || ch === '\t' || ch === '\r';
      if (isSpace) {
        if (!lastSpace) {
          out += ' ';
          lastSpace = true;
        }
        continue;
      }
      out += ch;
      lastSpace = false;
    }
    return out.trim();
  }

  private async waitForExpected(
    runtime: AgentSpaceRuntimeInfo,
    windowId: number,
    expected: UiTreeExpectedTarget,
    timeoutMs: number,
    pollIntervalMs: number
  ): Promise<boolean> {
    const expectedText = this.normalizeText(expected.text);
    const expectedIndex = this.normalizeIndex(expected.index);
    const startedAt = Date.now();
    while (Date.now() - startedAt <= timeoutMs) {
      const dump = await this.toolKit.getVisiblePageContent(windowId);
      const list = dump.trim().length > 0
        ? VisiblePageContentA11yTree.buildListFromDump(dump, runtime.widthPx, runtime.heightPx)
        : [];
      if (this.matchExpected(list, expectedText, expectedIndex)) {
        return true;
      }
      await sleep(pollIntervalMs);
    }
    return false;
  }

  private matchExpected(list: Array<A11yTreeListItem>, expectedText: string, expectedIndex: number): boolean {
    if (expectedText.length === 0) {
      if (expectedIndex <= 0) {
        return false;
      }
      for (let i = 0; i < list.length; i += 1) {
        if (list[i].index === expectedIndex) {
          return true;
        }
      }
      return false;
    }
    const candidates: Array<A11yTreeListItem> = [];
    for (let i = 0; i < list.length; i += 1) {
      const item = list[i];
      const text = this.normalizeText(item.text ?? '');
      if (text.length > 0 && text === expectedText) {
        candidates.push(item);
      }
    }
    if (candidates.length === 1) {
      return true;
    }
    if (candidates.length > 1 && expectedIndex > 0) {
      for (let i = 0; i < candidates.length; i += 1) {
        if (candidates[i].index === expectedIndex) {
          return true;
        }
      }
    }
    return false;
  }
}

function sleep(ms: number): Promise<void> {
  const delay = Math.max(0, Math.floor(ms));
  return new Promise<void>((resolve) => setTimeout(resolve, delay));
}
