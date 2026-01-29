export interface McpHello {
  type: string;
  device_id: string;
  session_id: string;
}

export interface McpToolCall {
  type: string;
  call_id: string;
  name: string;
  arguments?: Object;
}

export interface McpToolResultData {
}

export interface McpToolResultPayload {
  screenshot_b64: string;
  status: string;
  error: string | null;
  data: McpToolResultData;
}

export interface McpToolResult {
  type: string;
  call_id: string;
  result: McpToolResultPayload;
}
