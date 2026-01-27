import type { BusinessError } from '@kit.BasicServicesKit';
import AccessibilityExtensionAbility, {
  AccessibilityElement,
  ElementAttributeKeys,
  Rect
} from '@ohos.application.AccessibilityExtensionAbility';
import commonEventManager from '@ohos.commonEventManager';
import bundleManager from '@ohos.bundle.bundleManager';
import display from '@ohos.display';

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
  attributes?: Array<AccessibilityAttributeEntry>;
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

interface AccessibilityTreeListItem {
  index: number;
  resourceId: string;
  className: string;
  text: string;
  bounds: string;
  textCandidates: Array<string>;
  children: Array<AccessibilityTreeListItem>;
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

interface AccessibilityAttributeEntry {
  key: string;
  value: string;
}

type AttributeValue = string | number | boolean | Rect | Array<string> | Array<AccessibilityElement>;
type ReadableAttrValue = string | number | boolean | Rect | Array<string>;
type ReadableAttributeKey =
  | 'text'
  | 'accessibilityText'
  | 'contents'
  | 'description'
  | 'hintText'
  | 'resourceName'
  | 'componentId'
  | 'componentType'
  | 'customComponentType'
  | 'rect'
  | 'screenRect'
  | 'clickable'
  | 'isEnable'
  | 'isVisible'
  | 'isFocused'
  | 'selected'
  | 'checked'
  | 'editable'
  | 'scrollable'
  | 'bundleName'
  | 'windowId'
  | 'accessibilityFocused'
  | 'checkable'
  | 'textType'
  | 'valueMax'
  | 'valueMin'
  | 'valueNow';

interface ScreenSize {
  width: number;
  height: number;
}

interface ListPayload {
  data: string;
  truncated: boolean;
}

const DEFAULT_MAX_DEPTH: number = 10;
const DEFAULT_MAX_NODES: number = 120;
const MAX_PAYLOAD_LENGTH: number = 60000;
const DEFAULT_MIN_ELEMENT_SIZE: number = 5;

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
        maxDepth: this.normalizeBound(request.maxDepth, DEFAULT_MAX_DEPTH, 1, 200),
        maxNodes: this.normalizeBound(request.maxNodes, DEFAULT_MAX_NODES, 10, 3000),
        count: 0,
        truncated: false,
        nextId: 1
      };
      const output = (request.output ?? '').trim();
      if (output === 'list') {
        const screenSize = this.resolveScreenSize(displayId);
        const list: Array<AccessibilityTreeListItem> = [];
        await this.buildList(root, 0, state, list, screenSize, DEFAULT_MIN_ELEMENT_SIZE);
        const listPayload = this.stringifyList(list, state);
        return {
          requestId: request.requestId,
          ok: true,
          data: listPayload.data,
          truncated: listPayload.truncated
        };
      }
      const includeAttributes = request.includeAttributes === true;
      const tree = await this.buildNodeTree(root, 0, state, includeAttributes);
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
    const normalized = this.normalizeDisplayId(displayId);
    try {
      if (normalized === undefined) {
        return await ctx.getWindows();
      }
      return await ctx.getWindows(normalized);
    } catch (_e) {
      try {
        if (normalized === undefined) {
          return ctx.getAccessibilityWindowsSync();
        }
        return ctx.getAccessibilityWindowsSync(normalized);
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
    const normalized = this.normalizeDisplayId(windowId);
    try {
      if (normalized === undefined) {
        return await ctx.getWindowRootElement();
      }
      return await ctx.getWindowRootElement(normalized);
    } catch (_e) {
      if (ctx.getRootInActiveWindow) {
        try {
          if (normalized === undefined) {
            return await ctx.getRootInActiveWindow();
          }
          return await ctx.getRootInActiveWindow(normalized);
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
    state: TreeBuildState,
    includeAttributes: boolean
  ): Promise<AccessibilityTreeNode> {
    state.count += 1;
    const attrs = includeAttributes ? await this.collectReadableAttributes(element) : undefined;
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
      attributes: attrs
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
      nodes.push(await this.buildNodeTree(child, depth + 1, state, includeAttributes));
    }
    node.children = nodes;
    return node;
  }

  private async buildList(
    element: AccessibilityElement,
    depth: number,
    state: TreeBuildState,
    list: Array<AccessibilityTreeListItem>,
    screenSize: ScreenSize | null,
    minSize: number
  ): Promise<void> {
    if (depth > state.maxDepth || state.count >= state.maxNodes) {
      state.truncated = true;
      return;
    }
    const bounds = await this.readBoundsRect(element);
    if (!this.shouldIncludeElement(bounds, screenSize, minSize)) {
      return;
    }
    const text = await this.readStringAttr(element, 'text');
    const a11yText = await this.readStringAttr(element, 'accessibilityText');
    const contents = await this.readStringArrayAttr(element, 'contents');
    const description = await this.readStringAttr(element, 'description');
    const hintText = await this.readStringAttr(element, 'hintText');
    const resourceName = await this.readStringAttr(element, 'resourceName');
    const componentType = await this.readStringAttr(element, 'componentType');
    const customComponentType = await this.readStringAttr(element, 'customComponentType');
    const componentId = await this.readNumberAttr(element, 'componentId');
    const item: AccessibilityTreeListItem = {
      index: state.nextId,
      resourceId: this.pickResourceId(resourceName, componentId),
      className: this.shortClassName(customComponentType ?? componentType),
      text: this.pickText(text, a11yText, description, resourceName, customComponentType ?? componentType),
      bounds: this.formatBounds(bounds),
      textCandidates: this.buildTextCandidates(
        text,
        a11yText,
        description,
        contents,
        hintText,
        resourceName,
        customComponentType ?? componentType
      ),
      children: []
    };
    list.push(item);
    state.count += 1;
    state.nextId += 1;
    if (depth >= state.maxDepth || state.count >= state.maxNodes) {
      state.truncated = true;
      return;
    }
    const children = await this.listChildren(element);
    for (const child of children) {
      if (state.count >= state.maxNodes) {
        state.truncated = true;
        break;
      }
      await this.buildList(child, depth + 1, state, list, screenSize, minSize);
    }
  }

  private stringifyList(
    list: Array<AccessibilityTreeListItem>,
    state: TreeBuildState
  ): ListPayload {
    let data = JSON.stringify(list);
    if (data.length <= MAX_PAYLOAD_LENGTH) {
      const payload: ListPayload = { data, truncated: state.truncated };
      return payload;
    }
    let truncated = true;
    while (list.length > 0 && data.length > MAX_PAYLOAD_LENGTH) {
      list.pop();
      data = JSON.stringify(list);
    }
    const payload: ListPayload = { data, truncated };
    return payload;
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
      const raw = await element.attributeValue('children') as AttributeValue;
      if (Array.isArray(raw)) {
        return raw as Array<AccessibilityElement>;
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
      const raw = await element.attributeValue(key as ElementAttributeKeys) as AttributeValue;
      if (Array.isArray(raw)) {
        const results: Array<string> = [];
        for (const item of raw) {
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

  private async readBoundsRect(element: AccessibilityElement): Promise<Rect | undefined> {
    const screenRect = await this.readRectAttr(element, 'screenRect');
    if (screenRect) {
      return screenRect;
    }
    return await this.readRectAttr(element, 'rect');
  }

  private resolveScreenSize(displayId?: number): ScreenSize | null {
    const normalized = this.normalizeDisplayId(displayId);
    if (normalized === undefined) {
      return null;
    }
    try {
      const info = display.getDisplayByIdSync(normalized);
      if (info && Number.isFinite(info.width) && Number.isFinite(info.height)) {
        return { width: info.width, height: info.height };
      }
      return null;
    } catch (_e) {
      return null;
    }
  }

  private shouldIncludeElement(rect: Rect | undefined, screen: ScreenSize | null, minSize: number): boolean {
    if (!rect || !screen) {
      return true;
    }
    const left = rect.left;
    const top = rect.top;
    const width = rect.width;
    const height = rect.height;
    if (
      typeof left !== 'number' ||
      typeof top !== 'number' ||
      typeof width !== 'number' ||
      typeof height !== 'number'
    ) {
      return true;
    }
    if (width <= minSize || height <= minSize) {
      return false;
    }
    const right = left + width;
    const bottom = top + height;
    if (right <= 0 || bottom <= 0 || left >= screen.width || top >= screen.height) {
      return false;
    }
    return true;
  }

  private formatBounds(rect: Rect | undefined): string {
    if (!rect) {
      return '';
    }
    const left = rect.left;
    const top = rect.top;
    const width = rect.width;
    const height = rect.height;
    if (
      typeof left === 'number' &&
      typeof top === 'number' &&
      typeof width === 'number' &&
      typeof height === 'number'
    ) {
      return `${left},${top},${left + width},${top + height}`;
    }
    return '';
  }

  private shortClassName(typeName?: string): string {
    const name = (typeName ?? '').trim();
    if (name.length === 0) {
      return '';
    }
    const parts = name.split('.');
    return parts.length > 0 ? parts[parts.length - 1] : name;
  }

  private pickText(
    text: string | undefined,
    a11yText: string | undefined,
    description: string | undefined,
    resourceName: string | undefined,
    typeName: string | undefined
  ): string {
    const primary = (text ?? '').trim();
    if (primary.length > 0) {
      return primary;
    }
    const desc = (a11yText ?? description ?? '').trim();
    if (desc.length > 0) {
      return desc;
    }
    const resource = (resourceName ?? '').trim();
    if (resource.length > 0) {
      return resource;
    }
    return this.shortClassName(typeName);
  }

  private buildTextCandidates(
    text: string | undefined,
    a11yText: string | undefined,
    description: string | undefined,
    contents: Array<string> | undefined,
    hintText: string | undefined,
    resourceName: string | undefined,
    typeName: string | undefined
  ): Array<string> {
    const candidates: Array<string> = [];
    const primary = (text ?? '').trim();
    if (primary.length > 0) {
      candidates.push(`text="${primary}"`);
    }
    const a11y = (a11yText ?? '').trim();
    if (a11y.length > 0) {
      candidates.push(`a11yText="${a11y}"`);
    }
    const desc = (description ?? '').trim();
    if (desc.length > 0) {
      candidates.push(`desc="${desc}"`);
    }
    if (contents && contents.length > 0) {
      const joined = this.joinContents(contents);
      if (joined.length > 0) {
        candidates.push(`contents="${joined}"`);
      }
    }
    const hint = (hintText ?? '').trim();
    if (hint.length > 0) {
      candidates.push(`hint="${hint}"`);
    }
    const resource = (resourceName ?? '').trim();
    if (resource.length > 0) {
      candidates.push(`resource="${resource}"`);
    }
    const type = this.shortClassName(typeName);
    if (type.length > 0) {
      candidates.push(`class="${type}"`);
    }
    return candidates;
  }

  private pickResourceId(resourceName: string | undefined, componentId: number | undefined): string {
    const resource = (resourceName ?? '').trim();
    if (resource.length > 0) {
      return resource;
    }
    if (typeof componentId === 'number' && Number.isFinite(componentId)) {
      return `${componentId}`;
    }
    return '';
  }

  private joinContents(contents: Array<string>): string {
    const parts: Array<string> = [];
    for (const item of contents) {
      if (typeof item === 'string') {
        const trimmed = item.trim();
        if (trimmed.length > 0) {
          parts.push(trimmed);
        }
      }
    }
    return parts.join(' ');
  }

  private normalizeDisplayId(value?: number): number | undefined {
    if (typeof value !== 'number' || !Number.isFinite(value)) {
      return undefined;
    }
    const normalized = Math.trunc(value);
    if (normalized < 0) {
      return undefined;
    }
    return normalized;
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
      const raw = await element.attributeValue(key as ElementAttributeKeys) as AttributeValue;
      if (typeof raw === 'string' || typeof raw === 'number' || typeof raw === 'boolean') {
        return raw;
      }
      if (raw && typeof raw === 'object' && !Array.isArray(raw)) {
        return raw as Rect;
      }
      return undefined;
    } catch (_e) {
      return undefined;
    }
  }

  private async collectReadableAttributes(element: AccessibilityElement): Promise<Array<AccessibilityAttributeEntry>> {
    const keys: Array<ReadableAttributeKey> = [
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
    const attrs: Array<AccessibilityAttributeEntry> = [];
    for (const key of keys) {
      let value: ReadableAttrValue | undefined;
      try {
        const raw = await element.attributeValue(key as ElementAttributeKeys) as ReadableAttrValue;
        if (Array.isArray(raw)) {
          const parts: Array<string> = [];
          for (const item of raw) {
            if (typeof item === 'string' && item.length > 0) {
              parts.push(item);
            }
          }
          value = parts.length > 0 ? parts.join(' ') : undefined;
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
      attrs.push({
        key,
        value: this.formatAttrValue(value)
      });
    }
    return attrs;
  }

  private formatAttrValue(value: ReadableAttrValue): string {
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
