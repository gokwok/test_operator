import type { BusinessError } from '@kit.BasicServicesKit';
import AccessibilityExtensionAbility, {
  AccessibilityElement,
  ElementAttributeValues,
  Rect
} from '@kit.AccessibilityKit';
import commonEventManager, {
  CommonEventData,
  CommonEventPublishData,
  CommonEventSubscribeInfo,
  CommonEventSubscriber
} from '@ohos.commonEventManager';

import {
  ACCESS_TREE_BUNDLE_NAME,
  ACCESS_TREE_REQUEST_EVENT,
  ACCESS_TREE_RESPONSE_EVENT,
  AccessibilityTreeRequestPayload,
  AccessibilityTreeResponsePayload
} from '../features/agent_controller/accessibility/AccessibilityTreeProtocol';

type EventParamValue = string | number | boolean | undefined;
type EventParams = Record<string, EventParamValue>;

interface AccessibilityTreeNode {
  id: string;
  text?: string;
  description?: string;
  resourceName?: string;
  componentType?: string;
  rect?: Rect;
  screenRect?: Rect;
  clickable?: boolean;
  enabled?: boolean;
  visible?: boolean;
  focused?: boolean;
  selected?: boolean;
  checked?: boolean;
  editable?: boolean;
  scrollable?: boolean;
  bundleName?: string;
  windowId?: number;
  children: Array<AccessibilityTreeNode>;
}

interface AccessibilityWindowMeta {
  windowId?: number;
  bundleName?: string;
  type?: string;
  rect?: Rect;
}

interface AccessibilityTreeSnapshot {
  requestId: string;
  displayId?: number;
  targetWindowId?: number;
  windows: Array<AccessibilityWindowMeta>;
  tree?: AccessibilityTreeNode;
  nodeCount: number;
  truncated: boolean;
}

interface TreeBuildState {
  maxDepth: number;
  maxNodes: number;
  count: number;
  truncated: boolean;
  nextId: number;
}

interface WindowEntry {
  element: AccessibilityElement;
  meta: AccessibilityWindowMeta;
}

interface AccessibilityWindowContext {
  getAccessibilityWindowsSync(displayId?: number): Array<AccessibilityElement>;
  getRootInActiveWindow(windowId?: number): Promise<AccessibilityElement>;
  getWindowRootElement?(windowId?: number): Promise<AccessibilityElement>;
}

const DEFAULT_MAX_DEPTH: number = 10;
const DEFAULT_MAX_NODES: number = 120;
const MAX_PAYLOAD_LENGTH: number = 60000;

export default class AgentAccessibilityExtAbility extends AccessibilityExtensionAbility {
  private subscriber: CommonEventSubscriber | null = null;

  onConnect(): void {
    void this.handleConnect();
  }

  onDisconnect(): void {
    void this.handleDisconnect();
  }

  onAccessibilityConnect(): void {
    void this.handleConnect();
  }

  onAccessibilityDisconnect(): void {
    void this.handleDisconnect();
  }

  private async handleConnect(): Promise<void> {
    await this.ensureSubscriber();
  }

  private async handleDisconnect(): Promise<void> {
    await this.releaseSubscriber();
  }

  private async ensureSubscriber(): Promise<void> {
    if (this.subscriber) {
      return;
    }
    const info: CommonEventSubscribeInfo = {
      events: [ACCESS_TREE_REQUEST_EVENT]
    };
    const subscriber = await commonEventManager.createSubscriber(info);
    this.subscriber = subscriber;
    commonEventManager.subscribe(subscriber, (data: CommonEventData) => {
      void this.handleRequest(data);
    });
  }

  private async releaseSubscriber(): Promise<void> {
    const subscriber = this.subscriber;
    if (!subscriber) {
      return;
    }
    this.subscriber = null;
    commonEventManager.unsubscribe(subscriber, (_err: BusinessError | null) => {
      void _err;
    });
  }

  private async handleRequest(data: CommonEventData): Promise<void> {
    const params = this.toEventParams(data.parameters);
    const requestId = this.readString(params, 'requestId');
    if (requestId.length === 0) {
      return;
    }
    const request: AccessibilityTreeRequestPayload = {
      requestId,
      displayId: this.readNumber(params, 'displayId'),
      windowId: this.readNumber(params, 'windowId'),
      bundleName: this.readString(params, 'bundleName'),
      maxDepth: this.readNumber(params, 'maxDepth'),
      maxNodes: this.readNumber(params, 'maxNodes')
    };
    const response = await this.buildSnapshotResponse(request);
    this.publishResponse(response);
  }

  private publishResponse(response: AccessibilityTreeResponsePayload): void {
    const params: EventParams = {
      requestId: response.requestId,
      ok: response.ok,
      data: response.data,
      error: response.error,
      truncated: response.truncated
    };
    const publishData: CommonEventPublishData = {
      bundleName: ACCESS_TREE_BUNDLE_NAME,
      parameters: this.pruneParams(params)
    };
    commonEventManager.publish(ACCESS_TREE_RESPONSE_EVENT, publishData, (_err: BusinessError | null) => {
      void _err;
    });
  }

  private async buildSnapshotResponse(request: AccessibilityTreeRequestPayload): Promise<AccessibilityTreeResponsePayload> {
    try {
      const ctx = this.context;
      if (!ctx) {
        return {
          requestId: request.requestId,
          ok: false,
          error: 'context_unavailable'
        };
      }
      const displayId = this.normalizeDisplayId(request.displayId);
      const windows = this.listWindows(ctx, displayId);
      const entries = await this.buildWindowEntries(windows);
      const target = this.selectTargetWindow(entries, request);
      if (!target) {
        return {
          requestId: request.requestId,
          ok: false,
          error: 'window_not_found'
        };
      }
      const root = await this.getWindowRoot(ctx, target.meta.windowId);
      if (!root) {
        return {
          requestId: request.requestId,
          ok: false,
          error: 'root_not_found'
        };
      }
      const state: TreeBuildState = {
        maxDepth: this.normalizeBound(request.maxDepth, DEFAULT_MAX_DEPTH, 1, 30),
        maxNodes: this.normalizeBound(request.maxNodes, DEFAULT_MAX_NODES, 10, 1000),
        count: 0,
        truncated: false,
        nextId: 1
      };
      const tree = await this.buildNodeTree(root, 0, state);
      const snapshot: AccessibilityTreeSnapshot = {
        requestId: request.requestId,
        displayId: displayId,
        targetWindowId: target.meta.windowId,
        windows: entries.map((entry) => entry.meta),
        tree,
        nodeCount: state.count,
        truncated: state.truncated
      };
      let data = JSON.stringify(snapshot);
      if (data.length > MAX_PAYLOAD_LENGTH) {
        snapshot.tree = undefined;
        snapshot.truncated = true;
        data = JSON.stringify(snapshot);
      }
      return {
        requestId: request.requestId,
        ok: true,
        data,
        truncated: state.truncated
      };
    } catch (e) {
      return {
        requestId: request.requestId,
        ok: false,
        error: String(e)
      };
    }
  }

  private listWindows(
    ctx: AccessibilityWindowContext,
    displayId?: number
  ): Array<AccessibilityElement> {
    try {
      if (displayId === undefined) {
        return ctx.getAccessibilityWindowsSync();
      }
      return ctx.getAccessibilityWindowsSync(displayId);
    } catch (_e) {
      return [];
    }
  }

  private async buildWindowEntries(windows: Array<AccessibilityElement>): Promise<Array<WindowEntry>> {
    const entries: Array<WindowEntry> = [];
    for (const element of windows) {
      const meta = await this.buildWindowMeta(element);
      entries.push({ element, meta });
    }
    return entries;
  }

  private async buildWindowMeta(element: AccessibilityElement): Promise<AccessibilityWindowMeta> {
    const windowId = await this.readNumberAttr(element, 'windowId');
    const bundleName = await this.readStringAttr(element, 'bundleName');
    const type = await this.readStringAttr(element, 'type');
    const rect = await this.readRectAttr(element, 'rect');
    return {
      windowId,
      bundleName,
      type,
      rect
    };
  }

  private selectTargetWindow(
    entries: Array<WindowEntry>,
    request: AccessibilityTreeRequestPayload
  ): WindowEntry | null {
    if (entries.length === 0) {
      return null;
    }
    const targetWindowId = this.normalizeDisplayId(request.windowId);
    if (targetWindowId !== undefined) {
      for (const entry of entries) {
        if (entry.meta.windowId === targetWindowId) {
          return entry;
        }
      }
    }
    const bundleName = request.bundleName?.trim() ?? '';
    if (bundleName.length > 0) {
      for (const entry of entries) {
        if (entry.meta.bundleName === bundleName) {
          return entry;
        }
      }
    }
    for (const entry of entries) {
      if (entry.meta.type === 'application') {
        return entry;
      }
    }
    return entries[0];
  }

  private async getWindowRoot(
    ctx: AccessibilityWindowContext,
    windowId?: number
  ): Promise<AccessibilityElement | null> {
    try {
      return await ctx.getRootInActiveWindow(windowId);
    } catch (_e) {
      if (ctx.getWindowRootElement) {
        try {
          return await ctx.getWindowRootElement(windowId);
        } catch (_fallbackErr) {
          return null;
        }
      }
      return null;
    }
  }

  private async buildNodeTree(
    element: AccessibilityElement,
    depth: number,
    state: TreeBuildState
  ): Promise<AccessibilityTreeNode> {
    state.count += 1;
    const node: AccessibilityTreeNode = {
      id: String(state.nextId++),
      text: await this.readStringAttr(element, 'text'),
      description: await this.readStringAttr(element, 'description'),
      resourceName: await this.readStringAttr(element, 'resourceName'),
      componentType: await this.readStringAttr(element, 'componentType'),
      rect: await this.readRectAttr(element, 'rect'),
      screenRect: await this.readRectAttr(element, 'screenRect'),
      clickable: await this.readBooleanAttr(element, 'clickable'),
      enabled: await this.readBooleanAttr(element, 'isEnable'),
      visible: await this.readBooleanAttr(element, 'isVisible'),
      focused: await this.readBooleanAttr(element, 'isFocused'),
      selected: await this.readBooleanAttr(element, 'selected'),
      checked: await this.readBooleanAttr(element, 'checked'),
      editable: await this.readBooleanAttr(element, 'editable'),
      scrollable: await this.readBooleanAttr(element, 'scrollable'),
      bundleName: await this.readStringAttr(element, 'bundleName'),
      windowId: await this.readNumberAttr(element, 'windowId'),
      children: []
    };
    if (depth >= state.maxDepth || state.count >= state.maxNodes) {
      state.truncated = true;
      return node;
    }
    const children = await this.listChildren(element);
    const nodes: Array<AccessibilityTreeNode> = [];
    for (const child of children) {
      if (state.count >= state.maxNodes) {
        state.truncated = true;
        break;
      }
      nodes.push(await this.buildNodeTree(child, depth + 1, state));
    }
    node.children = nodes;
    return node;
  }

  private async listChildren(element: AccessibilityElement): Promise<Array<AccessibilityElement>> {
    try {
      return await element.getChildren();
    } catch (_e) {
      const fallback = await this.readAttr(element, 'children');
      return Array.isArray(fallback) ? fallback : [];
    }
  }

  private async readAttr<T extends keyof ElementAttributeValues>(
    element: AccessibilityElement,
    key: T
  ): Promise<ElementAttributeValues[T] | undefined> {
    try {
      return await element.attributeValue(key);
    } catch (_e) {
      return undefined;
    }
  }

  private async readStringAttr(
    element: AccessibilityElement,
    key: keyof ElementAttributeValues
  ): Promise<string | undefined> {
    const value = await this.readAttr(element, key);
    if (typeof value === 'string' && value.length > 0) {
      return value;
    }
    return undefined;
  }

  private async readNumberAttr(
    element: AccessibilityElement,
    key: keyof ElementAttributeValues
  ): Promise<number | undefined> {
    const value = await this.readAttr(element, key);
    if (typeof value === 'number' && Number.isFinite(value)) {
      return value;
    }
    return undefined;
  }

  private async readBooleanAttr(
    element: AccessibilityElement,
    key: keyof ElementAttributeValues
  ): Promise<boolean | undefined> {
    const value = await this.readAttr(element, key);
    if (typeof value === 'boolean') {
      return value;
    }
    return undefined;
  }

  private async readRectAttr(
    element: AccessibilityElement,
    key: keyof ElementAttributeValues
  ): Promise<Rect | undefined> {
    const value = await this.readAttr(element, key);
    if (value && typeof value === 'object') {
      return value as Rect;
    }
    return undefined;
  }

  private normalizeDisplayId(value?: number): number | undefined {
    if (value === undefined) {
      return undefined;
    }
    if (!Number.isFinite(value)) {
      return undefined;
    }
    return value;
  }

  private normalizeBound(value: number | undefined, fallback: number, min: number, max: number): number {
    if (!Number.isFinite(value)) {
      return fallback;
    }
    const clamped = Math.min(Math.max(value as number, min), max);
    return clamped;
  }

  private toEventParams(raw?: EventParams): EventParams {
    if (!raw) {
      return {};
    }
    return raw;
  }

  private pruneParams(params: EventParams): EventParams {
    const filtered: EventParams = {};
    for (const key of Object.keys(params)) {
      const value = params[key];
      if (value !== undefined && value !== '') {
        filtered[key] = value;
      }
    }
    return filtered;
  }

  private readString(params: EventParams, key: string): string {
    const value = params[key];
    if (typeof value === 'string') {
      return value;
    }
    if (typeof value === 'number') {
      return String(value);
    }
    return '';
  }

  private readNumber(params: EventParams, key: string): number | undefined {
    const value = params[key];
    if (typeof value === 'number' && Number.isFinite(value)) {
      return value;
    }
    if (typeof value === 'string') {
      const parsed = Number(value);
      return Number.isFinite(parsed) ? parsed : undefined;
    }
    return undefined;
  }
}
