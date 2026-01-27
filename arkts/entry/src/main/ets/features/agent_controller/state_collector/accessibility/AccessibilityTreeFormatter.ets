export interface AccessibilityTreeListItem {
  index: number;
  resourceId: string;
  className: string;
  text: string;
  bounds: string;
  textCandidates?: Array<string>;
  children: Array<AccessibilityTreeListItem>;
}

interface AccessibilityRect {
  left?: number;
  top?: number;
  right?: number;
  bottom?: number;
  width?: number;
  height?: number;
}

interface AccessibilityAttributeEntry {
  key: string;
  value: string;
}

interface AccessibilityTreeNode {
  text?: string;
  accessibilityText?: string;
  contents?: Array<string>;
  description?: string;
  hintText?: string;
  resourceName?: string;
  componentId?: number;
  componentType?: string;
  customComponentType?: string;
  rect?: AccessibilityRect;
  screenRect?: AccessibilityRect;
  attributes?: Array<AccessibilityAttributeEntry>;
  children?: Array<AccessibilityTreeNode>;
}

interface AccessibilityTreeSnapshot {
  tree?: AccessibilityTreeNode;
}

export class AccessibilityTreeFormatter {
  static toListJson(snapshotJson: string): string {
    const list = AccessibilityTreeFormatter.toList(snapshotJson);
    if (!list) {
      return 'ERR:accessibility_tree_parse_failed';
    }
    return JSON.stringify(list);
  }

  static toList(snapshotJson: string): Array<AccessibilityTreeListItem> | null {
    let snapshot: AccessibilityTreeSnapshot | null = null;
    try {
      snapshot = JSON.parse(snapshotJson) as AccessibilityTreeSnapshot;
    } catch (_e) {
      return null;
    }
    if (!snapshot) {
      return null;
    }
    const root = snapshot.tree;
    if (!root) {
      return [];
    }
    const results: Array<AccessibilityTreeListItem> = [];
    let index = 1;
    const walk = (node: AccessibilityTreeNode): void => {
      const item: AccessibilityTreeListItem = {
        index,
        resourceId: AccessibilityTreeFormatter.pickResourceId(node),
        className: AccessibilityTreeFormatter.shortClassName(node.customComponentType ?? node.componentType),
        text: AccessibilityTreeFormatter.pickText(node),
        bounds: AccessibilityTreeFormatter.formatBounds(node.screenRect ?? node.rect),
        textCandidates: AccessibilityTreeFormatter.buildTextCandidates(node),
        children: []
      };
      results.push(item);
      index += 1;
      const children = Array.isArray(node.children) ? node.children : [];
      for (const child of children) {
        walk(child);
      }
    };
    walk(root);
    return results;
  }

  static toDebugLines(snapshotJson: string): Array<string> | null {
    let snapshot: AccessibilityTreeSnapshot | null = null;
    try {
      snapshot = JSON.parse(snapshotJson) as AccessibilityTreeSnapshot;
    } catch (_e) {
      return null;
    }
    if (!snapshot || !snapshot.tree) {
      return [];
    }
    const lines: Array<string> = [];
    let index = 1;
    const walk = (node: AccessibilityTreeNode, depth: number): void => {
      const item: AccessibilityTreeListItem = {
        index,
        resourceId: AccessibilityTreeFormatter.pickResourceId(node),
        className: AccessibilityTreeFormatter.shortClassName(node.customComponentType ?? node.componentType),
        text: AccessibilityTreeFormatter.pickText(node),
        bounds: AccessibilityTreeFormatter.formatBounds(node.screenRect ?? node.rect),
        textCandidates: AccessibilityTreeFormatter.buildTextCandidates(node),
        children: []
      };
      AccessibilityTreeFormatter.appendNodeAttributes(lines, item, node.attributes, depth);
      index += 1;
      const children = Array.isArray(node.children) ? node.children : [];
      for (const child of children) {
        walk(child, depth + 1);
      }
    };
    walk(snapshot.tree, 0);
    return lines;
  }

  private static appendNodeAttributes(
    lines: Array<string>,
    item: AccessibilityTreeListItem,
    attributes: Array<AccessibilityAttributeEntry> | undefined,
    depth: number
  ): void {
    const indent = '  '.repeat(depth);
    const headerParts: Array<string> = [];
    headerParts.push(`${indent}${item.index}.`);
    if (item.className.length > 0) {
      headerParts.push(item.className);
    }
    if (item.resourceId.length > 0) {
      headerParts.push(`"${item.resourceId}"`);
    }
    if (item.text.length > 0) {
      headerParts.push(`"${item.text}"`);
    }
    if (item.bounds.length > 0) {
      headerParts.push(`(${item.bounds})`);
    }
    lines.push(headerParts.join(' '));
    if (!attributes) {
      return;
    }
    const ordered = attributes.slice().sort((a, b) => a.key.localeCompare(b.key));
    for (const entry of ordered) {
      if (entry.value.length === 0) {
        continue;
      }
      lines.push(`${indent}  - ${entry.key}: ${entry.value}`);
    }
  }


  private static pickText(node: AccessibilityTreeNode): string {
    const text = (node.text ?? '').trim();
    if (text.length > 0) {
      return text;
    }
    const a11yText = (node.accessibilityText ?? '').trim();
    if (a11yText.length > 0) {
      return a11yText;
    }
    const contents = AccessibilityTreeFormatter.joinContents(node.contents);
    if (contents.length > 0) {
      return contents;
    }
    const desc = (node.description ?? '').trim();
    if (desc.length > 0) {
      return desc;
    }
    const hint = (node.hintText ?? '').trim();
    if (hint.length > 0) {
      return hint;
    }
    const resource = (node.resourceName ?? '').trim();
    if (resource.length > 0) {
      return resource;
    }
    return (node.customComponentType ?? node.componentType ?? '').trim();
  }

  private static buildTextCandidates(node: AccessibilityTreeNode): Array<string> {
    const candidates: Array<string> = [];
    const text = (node.text ?? '').trim();
    if (text.length > 0) {
      candidates.push(`text="${text}"`);
    }
    const a11yText = (node.accessibilityText ?? '').trim();
    if (a11yText.length > 0) {
      candidates.push(`a11yText="${a11yText}"`);
    }
    const contents = AccessibilityTreeFormatter.joinContents(node.contents);
    if (contents.length > 0) {
      candidates.push(`contents="${contents}"`);
    }
    const desc = (node.description ?? '').trim();
    if (desc.length > 0) {
      candidates.push(`desc="${desc}"`);
    }
    const hint = (node.hintText ?? '').trim();
    if (hint.length > 0) {
      candidates.push(`hint="${hint}"`);
    }
    const resource = (node.resourceName ?? '').trim();
    if (resource.length > 0) {
      candidates.push(`resource="${resource}"`);
    }
    const type = (node.customComponentType ?? node.componentType ?? '').trim();
    if (type.length > 0) {
      candidates.push(`type="${type}"`);
    }
    return candidates;
  }

  private static pickResourceId(node: AccessibilityTreeNode): string {
    const resource = (node.resourceName ?? '').trim();
    if (resource.length > 0) {
      return resource;
    }
    const componentId = node.componentId;
    if (typeof componentId === 'number' && Number.isFinite(componentId)) {
      return `${componentId}`;
    }
    return '';
  }

  private static joinContents(contents?: Array<string>): string {
    if (!contents || contents.length === 0) {
      return '';
    }
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

  private static shortClassName(componentType?: string): string {
    if (!componentType) {
      return '';
    }
    const trimmed = componentType.trim();
    const dotIndex = trimmed.lastIndexOf('.');
    if (dotIndex >= 0 && dotIndex < trimmed.length - 1) {
      return trimmed.slice(dotIndex + 1);
    }
    return trimmed;
  }

  private static formatBounds(rect?: AccessibilityRect): string {
    if (!rect) {
      return '';
    }
    const left = rect.left;
    const top = rect.top;
    let right = rect.right;
    let bottom = rect.bottom;
    const width = rect.width;
    const height = rect.height;
    if (
      (typeof right !== 'number' || typeof bottom !== 'number') &&
      typeof width === 'number' &&
      typeof height === 'number' &&
      typeof left === 'number' &&
      typeof top === 'number'
    ) {
      right = left + width;
      bottom = top + height;
    }
    if (
      typeof left !== 'number' ||
      typeof top !== 'number' ||
      typeof right !== 'number' ||
      typeof bottom !== 'number'
    ) {
      return '';
    }
    if (!Number.isFinite(left) || !Number.isFinite(top) || !Number.isFinite(right) || !Number.isFinite(bottom)) {
      return '';
    }
    return `${left},${top},${right},${bottom}`;
  }
}
