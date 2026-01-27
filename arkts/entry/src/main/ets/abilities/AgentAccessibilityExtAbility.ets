import type { BusinessError } from '@kit.BasicServicesKit';
import AccessibilityExtensionAbility, {
  AccessibilityElement,
  ElementAttributeValues,
  Rect
} from '@ohos.application.AccessibilityExtensionAbility';
import commonEventManager from '@ohos.commonEventManager';
import bundleManager from '@ohos.bundle.bundleManager';

import {
  ACCESS_TREE_REQUEST_EVENT,
  ACCESS_TREE_RESPONSE_EVENT,
  AccessibilityTreeRequestPayload,
  AccessibilityTreeResponsePayload
} from '../features/agent_controller/state_collector/accessibility/AccessibilityTreeProtocol';

interface CommonEventDataSafe {
  data?: string;
}

interface CommonEventPublishDataSafe {
  bundleName?: string;
  data?: string;
}

interface AccessibilityTreeNode {
  id: string;
  text?: string;
  accessibilityText?: string;
  contents?: Array<string>;
  description?: string;
  hintText?: string;
  resourceName?: string;
  componentId?: number;
  componentType?: string;
  customComponentType?: string;
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
  attributes?: Record<string, string>;
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

const DEFAULT_MAX_DEPTH: number = 10;
const DEFAULT_MAX_NODES: number = 120;
const MAX_PAYLOAD_LENGTH: number = 60000;

export default class AgentAccessibilityExtAbility extends AccessibilityExtensionAbility {
  private subscriber: commonEventManager.CommonEventSubscriber | null = null;
  private cachedBundleName: string | null = null;

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
    const info: commonEventManager.CommonEventSubscribeInfo = {
      events: [ACCESS_TREE_REQUEST_EVENT]
    };
    const subscriber = await commonEventManager.createSubscriber(info);
    this.subscriber = subscriber;
    commonEventManager.subscribe(subscriber, (_err: BusinessError | null, data: object) => {
      void _err;
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

  private async handleRequest(data: object): Promise<void> {
    const payloadText = this.readEventData(data);
    if (payloadText.length === 0) {
      return;
    }
    let request: AccessibilityTreeRequestPayload | null = null;
    try {
      request = JSON.parse(payloadText) as AccessibilityTreeRequestPayload;
    } catch (_e) {
      return;
    }
    if (!request || request.requestId.length === 0) {
      return;
    }
    const response = await this.buildSnapshotResponse(request);
    this.publishResponse(response);
  }

  private publishResponse(response: AccessibilityTreeResponsePayload): void {
    const data = JSON.stringify(response);
    const publishData: CommonEventPublishDataSafe = {
      data
    };
    const bundleName = this.resolveSelfBundleName();
    if (bundleName.length > 0) {
      publishData.bundleName = bundleName;
    }
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
      const windows = await this.listWindows(displayId);
      const entries = await this.buildWindowEntries(windows);
      if (request.windowsOnly) {
        const snapshot: AccessibilityTreeSnapshot = {
          requestId: request.requestId,
          displayId: displayId,
          windows: entries.map((entry) => entry.meta),
          nodeCount: 0,
          truncated: false
        };
        const data = JSON.stringify(snapshot);
        return {
          requestId: request.requestId,
          ok: true,
          data,
          truncated: false
        };
      }
      const target = this.selectTargetWindow(entries, request);
      if (!target) {
        return {
          requestId: request.requestId,
          ok: false,
          error: 'window_not_found'
        };
      }
      const root = await this.getWindowRoot(target.meta.windowId);
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

  private async listWindows(displayId?: number): Promise<Array<AccessibilityElement>> {
    const ctx = this.context;
    if (!ctx) {
      return [];
    }
    try {
      return await ctx.getWindows(displayId);
    } catch (_e) {
      try {
        if (displayId === undefined) {
          return ctx.getAccessibilityWindowsSync();
        }
        return ctx.getAccessibilityWindowsSync(displayId);
      } catch (_fallbackErr) {
        return [];
      }
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

  private async getWindowRoot(windowId?: number): Promise<AccessibilityElement | null> {
    const ctx = this.context;
    if (!ctx) {
      return null;
    }
    try {
      return await ctx.getWindowRootElement(windowId);
    } catch (_e) {
      if (ctx.getRootInActiveWindow) {
        try {
          return await ctx.getRootInActiveWindow(windowId);
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
      accessibilityText: await this.readStringAttr(element, 'accessibilityText'),
      contents: await this.readStringArrayAttr(element, 'contents'),
      description: await this.readStringAttr(element, 'description'),
      hintText: await this.readStringAttr(element, 'hintText'),
      resourceName: await this.readStringAttr(element, 'resourceName'),
      componentId: await this.readNumberAttr(element, 'componentId'),
      componentType: await this.readStringAttr(element, 'componentType'),
      customComponentType: await this.readStringAttr(element, 'customComponentType'),
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
      children: [],
      attributes: await this.collectReadableAttributes(element)
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
    const fromAttr = await this.readChildrenAttr(element);
    if (fromAttr.length > 0) {
      return fromAttr;
    }
    try {
      return await element.getChildren();
    } catch (_e) {
      return [];
    }
  }

  private async readChildrenAttr(element: AccessibilityElement): Promise<Array<AccessibilityElement>> {
    try {
      const value = await element.attributeValue('children');
      if (Array.isArray(value)) {
        return value as Array<AccessibilityElement>;
      }
      return [];
    } catch (_e) {
      return [];
    }
  }

  private async readStringAttr(
    element: AccessibilityElement,
    key: string
  ): Promise<string | undefined> {
    const value = await this.readAttributeValue(element, key);
    if (typeof value === 'string' && value.length > 0) {
      return value;
    }
    return undefined;
  }

  private async readStringArrayAttr(
    element: AccessibilityElement,
    key: string
  ): Promise<Array<string> | undefined> {
    try {
      const value = await element.attributeValue(key as keyof ElementAttributeValues);
      if (Array.isArray(value)) {
        const results: Array<string> = [];
        for (const item of value) {
          if (typeof item === 'string' && item.length > 0) {
            results.push(item);
          }
        }
        return results.length > 0 ? results : undefined;
      }
      return undefined;
    } catch (_e) {
      return undefined;
    }
  }

  private async readNumberAttr(
    element: AccessibilityElement,
    key: string
  ): Promise<number | undefined> {
    const value = await this.readAttributeValue(element, key);
    if (typeof value === 'number' && Number.isFinite(value)) {
      return value;
    }
    return undefined;
  }

  private async readBooleanAttr(
    element: AccessibilityElement,
    key: string
  ): Promise<boolean | undefined> {
    const value = await this.readAttributeValue(element, key);
    if (typeof value === 'boolean') {
      return value;
    }
    return undefined;
  }

  private async readRectAttr(
    element: AccessibilityElement,
    key: string
  ): Promise<Rect | undefined> {
    const value = await this.readAttributeValue(element, key);
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

  private async readAttributeValue(
    element: AccessibilityElement,
    key: string
  ): Promise<string | number | boolean | Rect | undefined> {
    try {
      const value = await element.attributeValue(key as keyof ElementAttributeValues);
      if (typeof value === 'string' || typeof value === 'number' || typeof value === 'boolean') {
        return value;
      }
      if (value && typeof value === 'object') {
        return value as Rect;
      }
      return undefined;
    } catch (_e) {
      return undefined;
    }
  }

  private async collectReadableAttributes(element: AccessibilityElement): Promise<Record<string, string>> {
    const keys: Array<string> = [
      'text',
      'accessibilityText',
      'contents',
      'description',
      'hintText',
      'resourceName',
      'componentId',
      'componentType',
      'customComponentType',
      'rect',
      'screenRect',
      'clickable',
      'isEnable',
      'isVisible',
      'isFocused',
      'selected',
      'checked',
      'editable',
      'scrollable',
      'bundleName',
      'windowId',
      'accessibilityFocused',
      'checkable',
      'textType',
      'valueMax',
      'valueMin',
      'valueNow'
    ];
    const attrs: Record<string, string> = {};
    for (const key of keys) {
      let value: string | number | boolean | Rect | Array<string> | undefined;
      try {
        const raw = await element.attributeValue(key as keyof ElementAttributeValues);
        if (Array.isArray(raw)) {
          value = raw.filter((item) => typeof item === 'string').join(' ');
        } else if (raw && typeof raw === 'object') {
          value = raw as Rect;
        } else if (
          typeof raw === 'string' ||
          typeof raw === 'number' ||
          typeof raw === 'boolean'
        ) {
          value = raw;
        } else {
          value = undefined;
        }
      } catch (_e) {
        value = undefined;
      }
      if (value === undefined) {
        continue;
      }
      attrs[key] = this.formatAttrValue(value);
    }
    return attrs;
  }

  private formatAttrValue(value: string | number | boolean | Rect | Array<string>): string {
    if (typeof value === 'string') {
      return value;
    }
    if (typeof value === 'number' || typeof value === 'boolean') {
      return `${value}`;
    }
    if (Array.isArray(value)) {
      return value.join(' ');
    }
    const left = value.left;
    const top = value.top;
    const width = value.width;
    const height = value.height;
    if (
      typeof left === 'number' &&
      typeof top === 'number' &&
      typeof width === 'number' &&
      typeof height === 'number'
    ) {
      return `${left},${top},${left + width},${top + height}`;
    }
    return JSON.stringify(value);
  }

  private readEventData(data: object): string {
    const safe = data as CommonEventDataSafe;
    const text = safe.data;
    if (typeof text === 'string') {
      return text;
    }
    return '';
  }

  private resolveSelfBundleName(): string {
    if (this.cachedBundleName !== null) {
      return this.cachedBundleName;
    }
    let name = '';
    try {
      const info = bundleManager.getBundleInfoForSelfSync(bundleManager.BundleFlag.GET_BUNDLE_INFO_DEFAULT);
      name = info.name;
    } catch (_e) {
      name = '';
    }
    this.cachedBundleName = name;
    return name;
  }
}
