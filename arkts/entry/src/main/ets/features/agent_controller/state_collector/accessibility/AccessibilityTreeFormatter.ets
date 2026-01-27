export interface AccessibilityTreeListItem {
  index: number;
  resourceId: string;
  className: string;
  text: string;
  bounds: string;
  children: Array<AccessibilityTreeListItem>;
}

interface AccessibilityRect {
  left?: number;
  top?: number;
  right?: number;
  bottom?: number;
}

interface AccessibilityTreeNode {
  text?: string;
  accessibilityText?: string;
  description?: string;
  resourceName?: string;
  componentType?: string;
  rect?: AccessibilityRect;
  screenRect?: AccessibilityRect;
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
        resourceId: node.resourceName ?? '',
        className: AccessibilityTreeFormatter.shortClassName(node.componentType),
        text: AccessibilityTreeFormatter.pickText(node),
        bounds: AccessibilityTreeFormatter.formatBounds(node.screenRect ?? node.rect),
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

  private static pickText(node: AccessibilityTreeNode): string {
    const text = (node.text ?? '').trim();
    if (text.length > 0) {
      return text;
    }
    const a11yText = (node.accessibilityText ?? '').trim();
    if (a11yText.length > 0) {
      return a11yText;
    }
    const desc = (node.description ?? '').trim();
    if (desc.length > 0) {
      return desc;
    }
    const resource = (node.resourceName ?? '').trim();
    if (resource.length > 0) {
      return resource;
    }
    return (node.componentType ?? '').trim();
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
    const right = rect.right;
    const bottom = rect.bottom;
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
