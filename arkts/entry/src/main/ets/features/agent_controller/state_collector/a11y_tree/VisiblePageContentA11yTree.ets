export interface A11yTreeListItem {
  index: number;
  resourceId: string;
  className: string;
  text: string;
  bounds: string;
  textCandidates?: Array<string>;
  children: Array<A11yTreeListItem>;
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
    const filtered = VisiblePageContentA11yTree.filterNode(root, screenRect, options);
    if (!filtered) {
      return [];
    }
    return VisiblePageContentA11yTree.flatten(filtered);
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
      if (ch === '\\\\') {
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

  private static resolveScreenRect(root: VisiblePageContentNode, screenWidthPx: number, screenHeightPx: number): Rect {
    const rect = VisiblePageContentA11yTree.parseRect(root.$rect);
    if (rect) {
      return rect;
    }
    const width = Math.max(1, Math.floor(screenWidthPx));
    const height = Math.max(1, Math.floor(screenHeightPx));
    return { left: 0, top: 0, right: width, bottom: height };
  }

  private static filterNode(node: VisiblePageContentNode, screen: Rect, options: A11yTreeListOptions): FilteredNode | null {
    if (options.mode === A11yTreeFilterMode.CONCISE) {
      const decision = VisiblePageContentA11yTree.evaluateNode(node, screen, options);
      if (!decision.keepSelf) {
        return null;
      }
      const filteredChildren: Array<FilteredNode> = [];
      const children = Array.isArray(node.$children) ? node.$children : [];
      for (const child of children) {
        const filtered = VisiblePageContentA11yTree.filterNode(child, screen, options);
        if (filtered) {
          filteredChildren.push(filtered);
        }
      }
      return { node, rect: decision.rect, children: filteredChildren };
    }

    const filteredChildren: Array<FilteredNode> = [];
    const children = Array.isArray(node.$children) ? node.$children : [];
    for (const child of children) {
      const filtered = VisiblePageContentA11yTree.filterNode(child, screen, options);
      if (filtered) {
        filteredChildren.push(filtered);
      }
    }
    const decision = VisiblePageContentA11yTree.evaluateNode(node, screen, options);
    if (!decision.keepSelf && filteredChildren.length === 0) {
      return null;
    }
    return { node, rect: decision.rect, children: filteredChildren };
  }

  private static evaluateNode(
    node: VisiblePageContentNode,
    screen: Rect,
    options: A11yTreeListOptions
  ): { keepSelf: boolean; rect: Rect | null } {
    const rectRaw = VisiblePageContentA11yTree.parseRect(node.$rect);
    if (!rectRaw) {
      const hasChildren = Array.isArray(node.$children) && node.$children.length > 0;
      return { keepSelf: hasChildren, rect: null };
    }
    const rect = options.clipBounds ? VisiblePageContentA11yTree.clipRect(rectRaw, screen) : rectRaw;
    const width = rect.right - rect.left;
    const height = rect.bottom - rect.top;
    if (width <= options.minElementSizePx || height <= options.minElementSizePx) {
      return { keepSelf: false, rect };
    }
    if (node.visible === 'false') {
      return { keepSelf: false, rect };
    }
    const ignoreBoundsFiltering = VisiblePageContentA11yTree.isIgnoreBoundsFiltering(node.$attrs?.ignoreBoundsFiltering);
    if (!ignoreBoundsFiltering) {
      const intersection = VisiblePageContentA11yTree.intersect(rect, screen);
      const visibleArea = intersection ? VisiblePageContentA11yTree.area(intersection) : 0;
      const totalArea = VisiblePageContentA11yTree.area(rect);
      const ratio = totalArea > 0 ? visibleArea / totalArea : 0;
      if (ratio < options.visibilityThreshold) {
        return { keepSelf: false, rect };
      }
    }
    return { keepSelf: true, rect };
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
      const item: A11yTreeListItem = {
        index,
        resourceId: VisiblePageContentA11yTree.pickResourceId(node),
        className: VisiblePageContentA11yTree.pickClassName(node),
        text: VisiblePageContentA11yTree.pickText(node),
        bounds: VisiblePageContentA11yTree.formatBounds(entry.rect),
        textCandidates: VisiblePageContentA11yTree.buildTextCandidates(node),
        children: [],
      };
      results.push(item);
      index += 1;
      for (const child of entry.children) {
        walk(child);
      }
    };
    walk(root);
    return results;
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
    const match = raw.match(/\\[\\s*([0-9.+-]+)\\s*,\\s*([0-9.+-]+)\\s*\\]\\s*,\\s*\\[\\s*([0-9.+-]+)\\s*,\\s*([0-9.+-]+)\\s*\\]/);
    if (!match || match.length < 5) {
      return null;
    }
    const left = parseFloat(match[1]);
    const top = parseFloat(match[2]);
    const right = parseFloat(match[3]);
    const bottom = parseFloat(match[4]);
    if (!Number.isFinite(left) || !Number.isFinite(top) || !Number.isFinite(right) || !Number.isFinite(bottom)) {
      return null;
    }
    return { left, top, right, bottom };
  }

  private static clipRect(rect: Rect, screen: Rect): Rect {
    const left = Math.max(rect.left, screen.left);
    const top = Math.max(rect.top, screen.top);
    const right = Math.min(rect.right, screen.right);
    const bottom = Math.min(rect.bottom, screen.bottom);
    return { left, top, right, bottom };
  }

  private static intersect(a: Rect, b: Rect): Rect | null {
    const left = Math.max(a.left, b.left);
    const top = Math.max(a.top, b.top);
    const right = Math.min(a.right, b.right);
    const bottom = Math.min(a.bottom, b.bottom);
    if (right <= left || bottom <= top) {
      return null;
    }
    return { left, top, right, bottom };
  }

  private static area(rect: Rect): number {
    const width = rect.right - rect.left;
    const height = rect.bottom - rect.top;
    if (width <= 0 || height <= 0) {
      return 0;
    }
    return width * height;
  }
}

