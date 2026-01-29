import { ToolCallDispatcher, ToolCallResult } from '../tool_call/ToolCallDispatcher';
import type { McpHello, McpToolCall, McpToolResult, McpToolResultData, McpToolResultPayload } from './McpProtocol';
import { McpWsClient, McpWsClientListener } from './McpWsClient';
import type { ToolKit } from '../toolkit/ToolKit';

export type McpConnectionState = 'idle' | 'connecting' | 'connected' | 'closed' | 'error';

export interface McpStatus {
  state: McpConnectionState;
  url: string;
  error: string;
}

export interface DeviceToolsConfig {
  wsUrl: string;
  deviceId: string;
  sessionId: string;
  waitAfterActionMs: number;
}

export class DeviceToolsEndpoint {
  private toolKit: ToolKit;
  private client: McpWsClient = new McpWsClient();
  private dispatcher: ToolCallDispatcher | null = null;
  private status: McpStatus = { state: 'idle', url: '', error: '' };
  private config: DeviceToolsConfig | null = null;
  private callChain: Promise<void> = Promise.resolve();

  constructor(toolKit: ToolKit) {
    this.toolKit = toolKit;
    this.client.setListener(this.createListener());
  }

  async connect(config: DeviceToolsConfig): Promise<McpStatus> {
    const url = config.wsUrl.trim();
    if (url.length === 0) {
      this.status = { state: 'error', url: '', error: 'ws_url_empty' };
      return this.status;
    }
    this.config = config;
    this.status = { state: 'connecting', url, error: '' };
    this.dispatcher = new ToolCallDispatcher(this.toolKit, config.waitAfterActionMs);
    try {
      await this.client.connect(url);
    } catch (e) {
      this.status = { state: 'error', url, error: String(e) };
    }
    return this.status;
  }

  async disconnect(): Promise<McpStatus> {
    const url = this.status.url;
    try {
      await this.client.disconnect();
      this.status = { state: 'closed', url, error: '' };
    } catch (e) {
      this.status = { state: 'error', url, error: String(e) };
    }
    return this.status;
  }

  getStatus(): McpStatus {
    return this.status;
  }

  private createListener(): McpWsClientListener {
    const listener: McpWsClientListener = {
      onOpen: () => {
        this.status = { state: 'connected', url: this.status.url, error: '' };
        void this.sendHello();
      },
      onMessage: (message: string) => {
        const call = this.parseToolCall(message);
        if (!call) {
          return;
        }
        this.enqueueToolCall(call);
      },
      onClose: (_code: number, reason: string) => {
        this.status = { state: 'closed', url: this.status.url, error: reason };
      },
      onError: (message: string) => {
        this.status = { state: 'error', url: this.status.url, error: message };
      },
    };
    return listener;
  }

  private async sendHello(): Promise<void> {
    const cfg = this.config;
    if (!cfg) {
      return;
    }
    const payload: McpHello = {
      type: 'hello',
      device_id: cfg.deviceId,
      session_id: cfg.sessionId,
    };
    await this.client.sendText(JSON.stringify(payload));
  }

  private enqueueToolCall(call: McpToolCall): void {
    const next = this.callChain.then(async () => {
      await this.handleToolCall(call);
    }, async () => {
      await this.handleToolCall(call);
    });
    this.callChain = next.catch(() => {
      // keep chain alive
    });
  }

  private async handleToolCall(call: McpToolCall): Promise<void> {
    const dispatcher = this.dispatcher;
    if (!dispatcher) {
      return;
    }
    const argsJson = this.serializeArgs(call.arguments);
    const result = await dispatcher.execute(call.name, argsJson);
    const payload = this.buildToolResult(call.call_id, result);
    await this.client.sendText(JSON.stringify(payload));
  }

  private buildToolResult(callId: string, result: ToolCallResult): McpToolResult {
    const data: McpToolResultData = {};
    const payload: McpToolResultPayload = {
      screenshot_b64: result.screenshot_b64,
      status: result.status,
      error: result.error.length > 0 ? result.error : null,
      data,
    };
    const message: McpToolResult = {
      type: 'tool_result',
      call_id: callId,
      result: payload,
    };
    return message;
  }

  private parseToolCall(raw: string): McpToolCall | null {
    const trimmed = raw.trim();
    if (trimmed.length === 0) {
      return null;
    }
    try {
      const parsed: McpToolCall = JSON.parse(trimmed) as McpToolCall;
      if (!parsed || typeof parsed !== 'object') {
        return null;
      }
      if (parsed.type !== 'tool_call') {
        return null;
      }
      if (!parsed.call_id || !parsed.name) {
        return null;
      }
      return parsed;
    } catch (_e) {
      return null;
    }
  }

  private serializeArgs(args?: Object): string {
    if (!args) {
      return '{}';
    }
    try {
      return JSON.stringify(args);
    } catch (_e) {
      return '{}';
    }
  }
}
