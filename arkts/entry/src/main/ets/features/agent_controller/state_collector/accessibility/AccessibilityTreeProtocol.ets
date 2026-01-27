export const ACCESS_TREE_REQUEST_EVENT: string = 'com.huawei.aios.agent.operation.ACCESS_TREE_REQUEST';
export const ACCESS_TREE_RESPONSE_EVENT: string = 'com.huawei.aios.agent.operation.ACCESS_TREE_RESPONSE';
export const ACCESS_TREE_ABILITY_NAME: string = 'AgentAccessibilityExtAbility';

export interface AccessibilityTreeRequestOptions {
  displayId?: number;
  windowId?: number;
  bundleName?: string;
  maxDepth?: number;
  maxNodes?: number;
  windowsOnly?: boolean;
  includeAttributes?: boolean;
}

export interface AccessibilityTreeRequestPayload extends AccessibilityTreeRequestOptions {
  requestId: string;
}

export interface AccessibilityTreeResponsePayload {
  requestId: string;
  ok: boolean;
  data?: string;
  error?: string;
  truncated?: boolean;
}
