export interface A11yTreeListItem {
  index: number;
  resourceId: string;
  className: string;
  text: string;
  bounds: string;
  textCandidates?: Array<string>;
  children: Array<A11yTreeListItem>;
}

export interface A11yTreeHitTarget {
  text: string;
  index: number;
}

export enum A11yTreeFilterMode {
  CONCISE = 'concise',
  DETAILED = 'detailed',
}

export interface A11yTreeListOptions {
  mode: A11yTreeFilterMode;
  minElementSizePx: number;
  visibilityThreshold: number;
  clipBounds: boolean;
  filterKeyboard: boolean;
  keyboardResourceIdPrefixes: Array<string>;
}

interface VisiblePageContentAppInfo {
  BundleName?: string;
  WindowID?: number;
  WindowName?: string;
  CurrentPageUrl?: string;
  CurrentPageName?: string;
}

interface VisiblePageContentAttrs {
  content?: string;
  description?: string;
  accessibilityText?: string;
  hintText?: string;
  ignoreBoundsFiltering?: boolean | string;
}

interface VisiblePageContentNode {
  appInfo?: VisiblePageContentAppInfo;
  $type?: string;
  $ID?: number;
  type?: string;
  compid?: string;
  $rect?: string;
  $attrs?: VisiblePageContentAttrs;
  clickable?: boolean;
  longClickable?: boolean;
  scrollable?: boolean;
  editable?: boolean;
  visible?: string;
  $children?: Array<VisiblePageContentNode>;
}

interface Rect {
  left: number;
  top: number;
  right: number;
  bottom: number;
}

class EvalDecision {
  keepSelf: boolean;
  rect: Rect | null;

  constructor(keepSelf: boolean, rect: Rect | null) {
    this.keepSelf = keepSelf;
    this.rect = rect;
  }
}

interface FilteredNode {
  node: VisiblePageContentNode;
  rect: Rect | null;
  children: Array<FilteredNode>;
}

const DEFAULT_MIN_ELEMENT_SIZE_PX: number = 5;
const DEFAULT_VISIBILITY_THRESHOLD: number = 0.1;

export class VisiblePageContentA11yTree {
  static readonly DEFAULT_OPTIONS: A11yTreeListOptions = {
    mode: A11yTreeFilterMode.DETAILED,
    minElementSizePx: DEFAULT_MIN_ELEMENT_SIZE_PX,
    visibilityThreshold: DEFAULT_VISIBILITY_THRESHOLD,
    clipBounds: false,
    filterKeyboard: false,
    keyboardResourceIdPrefixes: [],
  };

  static buildListFromDump(
    dumpJson: string,
    screenWidthPx: number,
    screenHeightPx: number,
    options: A11yTreeListOptions = VisiblePageContentA11yTree.DEFAULT_OPTIONS
  ): Array<A11yTreeListItem> {
    const root = VisiblePageContentA11yTree.parseDumpRoot(dumpJson);
    if (!root) {
      return [];
    }
    const screenRect = VisiblePageContentA11yTree.resolveScreenRect(root, screenWidthPx, screenHeightPx);
    const filtered = options.mode === A11yTreeFilterMode.CONCISE
      ? VisiblePageContentA11yTree.filterConcise(root, screenRect, options)
      : VisiblePageContentA11yTree.filterDetailed(root, screenRect, options);
    if (!filtered) {
      return [];
    }
    return VisiblePageContentA11yTree.flatten(filtered);
  }

  static hitTargetAtPoint(list: Array<A11yTreeListItem>, xPx: number, yPx: number): A11yTreeHitTarget {
    let bestText: string = '';
    let bestIndex: number = 0;
    let bestArea: number = Number.POSITIVE_INFINITY;
    for (const item of list) {
      const rect = VisiblePageContentA11yTree.parseBounds(item.bounds);
      if (!rect) {
        continue;
      }
      if (xPx < rect.left || xPx > rect.right || yPx < rect.top || yPx > rect.bottom) {
        continue;
      }
      const area = Math.max(0, rect.right - rect.left) * Math.max(0, rect.bottom - rect.top);
      const text = (item.text ?? '').trim();
      if (bestText.length === 0 && text.length > 0) {
        bestText = text;
        bestIndex = item.index;
        bestArea = area;
        continue;
      }
      if (text.length === 0 && bestText.length > 0) {
        continue;
      }
      if (area > 0 && area < bestArea) {
        bestText = text;
        bestIndex = item.index;
        bestArea = area;
      }
    }
    return { text: bestText, index: bestIndex };
  }

  static hitTextAtPoint(list: Array<A11yTreeListItem>, xPx: number, yPx: number): string {
    return VisiblePageContentA11yTree.hitTargetAtPoint(list, xPx, yPx).text;
  }

  private static parseDumpRoot(dumpJson: string): VisiblePageContentNode | null {
    const encoded = VisiblePageContentA11yTree.extractFirstObjectStringValue(dumpJson);
    if (encoded.length === 0) {
      return null;
    }
    let innerJson: string;
    try {
      innerJson = JSON.parse(`\"${encoded}\"`) as string;
    } catch (_e) {
      return null;
    }
    try {
      return JSON.parse(innerJson) as VisiblePageContentNode;
    } catch (_e) {
      return null;
    }
  }

  private static extractFirstObjectStringValue(json: string): string {
    const trimmed = json.trim();
    const colonIndex = trimmed.indexOf(':');
    if (colonIndex < 0) {
      return '';
    }
    let i = colonIndex + 1;
    while (i < trimmed.length && trimmed.charAt(i) !== '"') {
      i += 1;
    }
    if (i >= trimmed.length) {
      return '';
    }
    const start = i + 1;
    i = start;
    let escaped = false;
    while (i < trimmed.length) {
      const ch = trimmed.charAt(i);
      if (escaped) {
        escaped = false;
        i += 1;
        continue;
      }
      if (ch === '\\') {
        escaped = true;
        i += 1;
        continue;
      }
      if (ch === '"') {
        return trimmed.slice(start, i);
      }
      i += 1;
    }
    return '';
  }

  private static createRect(left: number, top: number, right: number, bottom: number): Rect {
    const rect: Rect = { left, top, right, bottom };
    return rect;
  }

  private static resolveScreenRect(root: VisiblePageContentNode, screenWidthPx: number, screenHeightPx: number): Rect {
    const rect = VisiblePageContentA11yTree.parseRect(root.$rect);
    if (rect) {
      return rect;
    }
    const width = Math.max(1, Math.floor(screenWidthPx));
    const height = Math.max(1, Math.floor(screenHeightPx));
    return VisiblePageContentA11yTree.createRect(0, 0, width, height);
  }

  private static filterConcise(node: VisiblePageContentNode, screen: Rect, options: A11yTreeListOptions): FilteredNode | null {
    const decision = VisiblePageContentA11yTree.evaluateConciseNode(node, screen, options);
    if (!decision.keepSelf) {
      return null;
    }
    const filteredChildren: Array<FilteredNode> = [];
    const children = Array.isArray(node.$children) ? node.$children : [];
    for (const child of children) {
      const filtered = VisiblePageContentA11yTree.filterConcise(child, screen, options);
      if (filtered) {
        filteredChildren.push(filtered);
      }
    }
    return { node, rect: decision.rect, children: filteredChildren };
  }

  private static filterDetailed(node: VisiblePageContentNode, screen: Rect, options: A11yTreeListOptions): FilteredNode | null {
    const input = options.filterKeyboard ? VisiblePageContentA11yTree.filterKeyboardElements(node, options) : node;
    if (!input) {
      return null;
    }
    return VisiblePageContentA11yTree.filterOutOfBoundsDetailed(input, screen, options);
  }

  private static filterKeyboardElements(node: VisiblePageContentNode, options: A11yTreeListOptions): VisiblePageContentNode | null {
    const filteredChildren: Array<VisiblePageContentNode> = [];
    const children = Array.isArray(node.$children) ? node.$children : [];
    for (const child of children) {
      const filteredChild = VisiblePageContentA11yTree.filterKeyboardElements(child, options);
      if (filteredChild) {
        filteredChildren.push(filteredChild);
      }
    }
    if (VisiblePageContentA11yTree.shouldFilterKeyboard(node, options.keyboardResourceIdPrefixes)) {
      return null;
    }
    const cloned: VisiblePageContentNode = {
      appInfo: node.appInfo,
      $type: node.$type,
      $ID: node.$ID,
      type: node.type,
      compid: node.compid,
      $rect: node.$rect,
      $attrs: node.$attrs,
      clickable: node.clickable,
      longClickable: node.longClickable,
      scrollable: node.scrollable,
      editable: node.editable,
      visible: node.visible,
      $children: filteredChildren,
    };
    return cloned;
  }

  private static shouldFilterKeyboard(node: VisiblePageContentNode, prefixes: Array<string>): boolean {
    if (!prefixes || prefixes.length === 0) {
      return false;
    }
    const resourceId = (node.compid ?? '').trim();
    if (resourceId.length === 0) {
      return false;
    }
    for (const prefix of prefixes) {
      const p = prefix.trim();
      if (p.length > 0 && resourceId.startsWith(p)) {
        return true;
      }
    }
    return false;
  }

  private static filterOutOfBoundsDetailed(node: VisiblePageContentNode, screen: Rect, options: A11yTreeListOptions): FilteredNode | null {
    const ignoreBoundsFiltering = VisiblePageContentA11yTree.isIgnoreBoundsFiltering(node.$attrs?.ignoreBoundsFiltering);
    if (ignoreBoundsFiltering) {
      return VisiblePageContentA11yTree.buildFullSubtree(node, screen, options);
    }

    const filteredChildren: Array<FilteredNode> = [];
    const children = Array.isArray(node.$children) ? node.$children : [];
    for (const child of children) {
      const filtered = VisiblePageContentA11yTree.filterOutOfBoundsDetailed(child, screen, options);
      if (filtered) {
        filteredChildren.push(filtered);
      }
    }

    const rectRaw = VisiblePageContentA11yTree.parseRect(node.$rect);
    const rect = rectRaw && options.clipBounds ? VisiblePageContentA11yTree.clipRect(rectRaw, screen) : rectRaw;
    const visiblePercentage = rect ? VisiblePageContentA11yTree.getVisiblePercentage(rect, screen) : 0;
    const keepSelf = visiblePercentage >= options.visibilityThreshold;
    if (!keepSelf && filteredChildren.length === 0) {
      return null;
    }
    if (node.visible === 'false' && filteredChildren.length === 0) {
      return null;
    }
    return { node, rect: rect ?? null, children: filteredChildren };
  }

  private static buildFullSubtree(node: VisiblePageContentNode, screen: Rect, options: A11yTreeListOptions): FilteredNode {
    const rectRaw = VisiblePageContentA11yTree.parseRect(node.$rect);
    const rect = rectRaw && options.clipBounds ? VisiblePageContentA11yTree.clipRect(rectRaw, screen) : rectRaw;
    const fullChildren: Array<FilteredNode> = [];
    const children = Array.isArray(node.$children) ? node.$children : [];
    for (const child of children) {
      fullChildren.push(VisiblePageContentA11yTree.buildFullSubtree(child, screen, options));
    }
    return { node, rect: rect ?? null, children: fullChildren };
  }

  private static evaluateConciseNode(
    node: VisiblePageContentNode,
    screen: Rect,
    options: A11yTreeListOptions
  ): EvalDecision {
    const rectRaw = VisiblePageContentA11yTree.parseRect(node.$rect);
    if (!rectRaw) {
      const hasChildren = Array.isArray(node.$children) && node.$children.length > 0;
      return new EvalDecision(hasChildren, null);
    }
    const rect = options.clipBounds ? VisiblePageContentA11yTree.clipRect(rectRaw, screen) : rectRaw;
    const width = rect.right - rect.left;
    const height = rect.bottom - rect.top;
    if (width <= options.minElementSizePx || height <= options.minElementSizePx) {
      return new EvalDecision(false, rect);
    }
    if (node.visible === 'false') {
      return new EvalDecision(false, rect);
    }
    const intersection = VisiblePageContentA11yTree.intersect(rect, screen);
    if (!intersection) {
      return new EvalDecision(false, rect);
    }
    return new EvalDecision(true, rect);
  }

  private static isIgnoreBoundsFiltering(value: boolean | string | undefined): boolean {
    if (value === true) {
      return true;
    }
    if (typeof value === 'string') {
      return value.trim().toLowerCase() === 'true';
    }
    return false;
  }

  private static flatten(root: FilteredNode): Array<A11yTreeListItem> {
    const results: Array<A11yTreeListItem> = [];
    let index = 1;
    const walk = (entry: FilteredNode): void => {
      const node = entry.node;
      if (VisiblePageContentA11yTree.isOperable(node)) {
        const item: A11yTreeListItem = {
          index,
          resourceId: VisiblePageContentA11yTree.pickResourceId(node),
          className: VisiblePageContentA11yTree.pickClassName(node),
          text: VisiblePageContentA11yTree.pickOperableText(entry),
          bounds: VisiblePageContentA11yTree.formatBounds(entry.rect),
          textCandidates: VisiblePageContentA11yTree.buildTextCandidates(node),
          children: [],
        };
        results.push(item);
        index += 1;
      }
      for (const child of entry.children) {
        walk(child);
      }
    };
    walk(root);
    return results;
  }

  private static isOperable(node: VisiblePageContentNode): boolean {
    return node.clickable === true ||
      node.longClickable === true ||
      node.scrollable === true ||
      node.editable === true;
  }

  private static pickOperableText(entry: FilteredNode): string {
    const node = entry.node;
    if (node.clickable === true || node.longClickable === true) {
      return VisiblePageContentA11yTree.pickMergedText(entry);
    }
    return VisiblePageContentA11yTree.pickText(node);
  }

  private static pickMergedText(entry: FilteredNode): string {
    const direct = VisiblePageContentA11yTree.pickPrimaryText(entry.node);
    if (direct.length > 0) {
      return direct;
    }
    const merged = VisiblePageContentA11yTree.findPrimaryTextInSubtree(entry);
    if (merged.length > 0) {
      return merged;
    }
    return VisiblePageContentA11yTree.pickText(entry.node);
  }

  private static pickPrimaryText(node: VisiblePageContentNode): string {
    const text = (node.$attrs?.content ?? '').trim();
    if (text.length > 0) {
      return text;
    }
    const desc = (node.$attrs?.accessibilityText ?? node.$attrs?.description ?? '').trim();
    if (desc.length > 0) {
      return desc;
    }
    const hint = (node.$attrs?.hintText ?? '').trim();
    if (hint.length > 0) {
      return hint;
    }
    return '';
  }

  private static findPrimaryTextInSubtree(entry: FilteredNode): string {
    for (const child of entry.children) {
      const direct = VisiblePageContentA11yTree.pickPrimaryText(child.node);
      if (direct.length > 0) {
        return direct;
      }
      const nested = VisiblePageContentA11yTree.findPrimaryTextInSubtree(child);
      if (nested.length > 0) {
        return nested;
      }
    }
    return '';
  }

  private static pickResourceId(node: VisiblePageContentNode): string {
    const compid = (node.compid ?? '').trim();
    if (compid.length > 0) {
      return compid;
    }
    const id = node.$ID;
    if (typeof id === 'number' && Number.isFinite(id)) {
      return `${id}`;
    }
    return '';
  }

  private static pickClassName(node: VisiblePageContentNode): string {
    const t = (node.$type ?? '').trim();
    if (t.length > 0) {
      return t;
    }
    const rawType = (node.type ?? '').trim();
    if (rawType.length > 0) {
      return rawType;
    }
    return '';
  }

  private static pickText(node: VisiblePageContentNode): string {
    const text = (node.$attrs?.content ?? '').trim();
    if (text.length > 0) {
      return text;
    }
    const desc = (node.$attrs?.accessibilityText ?? node.$attrs?.description ?? '').trim();
    if (desc.length > 0) {
      return desc;
    }
    const hint = (node.$attrs?.hintText ?? '').trim();
    if (hint.length > 0) {
      return hint;
    }
    const resource = (node.compid ?? '').trim();
    if (resource.length > 0) {
      return resource;
    }
    return VisiblePageContentA11yTree.pickClassName(node);
  }

  private static buildTextCandidates(node: VisiblePageContentNode): Array<string> {
    const candidates: Array<string> = [];
    const text = (node.$attrs?.content ?? '').trim();
    if (text.length > 0) {
      candidates.push(`text=\"${text}\"`);
    }
    const a11yText = (node.$attrs?.accessibilityText ?? '').trim();
    if (a11yText.length > 0) {
      candidates.push(`a11yText=\"${a11yText}\"`);
    }
    const desc = (node.$attrs?.description ?? '').trim();
    if (desc.length > 0) {
      candidates.push(`desc=\"${desc}\"`);
    }
    const hint = (node.$attrs?.hintText ?? '').trim();
    if (hint.length > 0) {
      candidates.push(`hint=\"${hint}\"`);
    }
    const resource = (node.compid ?? '').trim();
    if (resource.length > 0) {
      candidates.push(`compid=\"${resource}\"`);
    }
    const className = VisiblePageContentA11yTree.pickClassName(node);
    if (className.length > 0) {
      candidates.push(`class=\"${className}\"`);
    }
    if (node.clickable === true) {
      candidates.push('clickable=true');
    }
    if (node.longClickable === true) {
      candidates.push('longClickable=true');
    }
    if (node.scrollable === true) {
      candidates.push('scrollable=true');
    }
    if (node.editable === true) {
      candidates.push('editable=true');
    }
    return candidates;
  }

  private static formatBounds(rect: Rect | null): string {
    if (!rect) {
      return '';
    }
    const left = Math.round(rect.left);
    const top = Math.round(rect.top);
    const right = Math.round(rect.right);
    const bottom = Math.round(rect.bottom);
    return `${left},${top},${right},${bottom}`;
  }

  private static parseRect(rectText?: string): Rect | null {
    const raw = (rectText ?? '').trim();
    if (raw.length === 0) {
      return null;
    }
    const numbers: Array<number> = [];
    let token: string = '';
    const flush = (): void => {
      if (token.length === 0) {
        return;
      }
      const value = parseFloat(token);
      token = '';
      if (Number.isFinite(value)) {
        numbers.push(value);
      }
    };
    for (let i = 0; i < raw.length; i += 1) {
      const ch = raw.charAt(i);
      const isDigit = ch >= '0' && ch <= '9';
      if (isDigit || ch === '.' || ch === '-' || ch === '+') {
        token += ch;
        continue;
      }
      flush();
      if (numbers.length >= 4) {
        break;
      }
    }
    flush();
    if (numbers.length < 4) {
      return null;
    }
    const left = numbers[0];
    const top = numbers[1];
    const right = numbers[2];
    const bottom = numbers[3];
    if (!Number.isFinite(left) || !Number.isFinite(top) || !Number.isFinite(right) || !Number.isFinite(bottom)) {
      return null;
    }
    return VisiblePageContentA11yTree.createRect(left, top, right, bottom);
  }

  private static parseBounds(bounds: string): Rect | null {
    const trimmed = bounds.trim();
    if (trimmed.length === 0) {
      return null;
    }
    const parts = trimmed.split(',');
    if (parts.length < 4) {
      return null;
    }
    const left = parseFloat(parts[0]);
    const top = parseFloat(parts[1]);
    const right = parseFloat(parts[2]);
    const bottom = parseFloat(parts[3]);
    if (!Number.isFinite(left) || !Number.isFinite(top) || !Number.isFinite(right) || !Number.isFinite(bottom)) {
      return null;
    }
    return VisiblePageContentA11yTree.createRect(left, top, right, bottom);
  }

  private static clipRect(rect: Rect, screen: Rect): Rect {
    const left = Math.max(rect.left, screen.left);
    const top = Math.max(rect.top, screen.top);
    const right = Math.min(rect.right, screen.right);
    const bottom = Math.min(rect.bottom, screen.bottom);
    return VisiblePageContentA11yTree.createRect(left, top, right, bottom);
  }

  private static intersect(a: Rect, b: Rect): Rect | null {
    const left = Math.max(a.left, b.left);
    const top = Math.max(a.top, b.top);
    const right = Math.min(a.right, b.right);
    const bottom = Math.min(a.bottom, b.bottom);
    if (right <= left || bottom <= top) {
      return null;
    }
    return VisiblePageContentA11yTree.createRect(left, top, right, bottom);
  }

  private static area(rect: Rect): number {
    const width = rect.right - rect.left;
    const height = rect.bottom - rect.top;
    if (width <= 0 || height <= 0) {
      return 0;
    }
    return width * height;
  }

  private static getVisiblePercentage(rect: Rect, screen: Rect): number {
    const width = rect.right - rect.left;
    const height = rect.bottom - rect.top;
    if (width === 0 && height === 0) {
      return 0;
    }
    const overflow = rect.left <= screen.left && rect.top <= screen.top && rect.right >= screen.right && rect.bottom >= screen.bottom;
    if (overflow) {
      return 1;
    }
    const visibleX = Math.max(0, Math.min(rect.right, screen.right) - Math.max(rect.left, screen.left));
    const visibleY = Math.max(0, Math.min(rect.bottom, screen.bottom) - Math.max(rect.top, screen.top));
    const visibleArea = visibleX * visibleY;
    const totalArea = width * height;
    return totalArea > 0 ? visibleArea / totalArea : 0;
  }
}
