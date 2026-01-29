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

export type McpLogFn = (message: string) => void;

export class DeviceToolsEndpoint {
  private toolKit: ToolKit;
  private client: McpWsClient = new McpWsClient();
  private dispatcher: ToolCallDispatcher | null = null;
  private status: McpStatus = { state: 'idle', url: '', error: '' };
  private config: DeviceToolsConfig | null = null;
  private callChain: Promise<void> = Promise.resolve();
  private logger: McpLogFn | null = null;

  constructor(toolKit: ToolKit, logger?: McpLogFn) {
    this.toolKit = toolKit;
    this.logger = logger ?? null;
    this.client.setListener(new DeviceToolsEndpointListener(this));
  }

  async connect(config: DeviceToolsConfig): Promise<McpStatus> {
    const url = config.wsUrl.trim();
    if (url.length === 0) {
      this.status = { state: 'error', url: '', error: 'ws_url_empty' };
      this.log('connect_failed: ws_url_empty');
      return this.status;
    }
    this.config = config;
    this.status = { state: 'connecting', url, error: '' };
    this.dispatcher = new ToolCallDispatcher(this.toolKit, config.waitAfterActionMs);
    this.log(`connecting ws=${url}`);
    try {
      await this.client.connect(url);
    } catch (e) {
      this.status = { state: 'error', url, error: String(e) };
      this.log(`connect_error: ${String(e)}`);
    }
    return this.status;
  }

  async disconnect(): Promise<McpStatus> {
    const url = this.status.url;
    try {
      await this.client.disconnect();
      this.status = { state: 'closed', url, error: '' };
      this.log('disconnected');
    } catch (e) {
      this.status = { state: 'error', url, error: String(e) };
      this.log(`disconnect_error: ${String(e)}`);
    }
    return this.status;
  }

  getStatus(): McpStatus {
    return this.status;
  }

  handleOpen(): void {
    this.status = { state: 'connected', url: this.status.url, error: '' };
    this.log('ws_open');
    void this.sendHello();
  }

  handleMessage(message: string): void {
    const call = this.parseToolCall(message);
    if (!call) {
      this.log(`recv_ignored: ${this.truncate(message, 120)}`);
      return;
    }
    this.log(`recv_tool_call: name=${call.name} call_id=${call.call_id}`);
    this.enqueueToolCall(call);
  }

  handleClose(_code: number, reason: string): void {
    this.status = { state: 'closed', url: this.status.url, error: reason };
    this.log(`ws_close: ${reason}`);
  }

  handleError(message: string): void {
    this.status = { state: 'error', url: this.status.url, error: message };
    this.log(`ws_error: ${message}`);
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
    this.log(`send_hello: device_id=${cfg.deviceId} session_id=${cfg.sessionId}`);
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
      this.log('dispatcher_missing');
      return;
    }
    const argsJson = this.serializeArgs(call.arguments);
    this.log(`exec_tool: ${call.name} args=${this.truncate(argsJson, 180)}`);
    const result = await dispatcher.execute(call.name, argsJson);
    const payload = this.buildToolResult(call.call_id, result);
    this.log(
      `send_result: call_id=${call.call_id} status=${result.status} screenshot_len=${result.screenshot_b64.length} error=${this.truncate(result.error, 120)}`
    );
    try {
      await this.client.sendText(JSON.stringify(payload));
    } catch (e) {
      this.log(`send_result_error: ${String(e)}`);
    }
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
    const trimmed = this.normalizeRawMessage(raw);
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

  private normalizeRawMessage(raw: string): string {
    let trimmed = raw.trim();
    if (trimmed.length === 0) {
      return '';
    }
    if (this.hasWsRawPrefix(trimmed)) {
      trimmed = this.stripWsRawPrefix(trimmed).trim();
    }
    if (trimmed.startsWith('"') && trimmed.endsWith('"')) {
      try {
        const inner = JSON.parse(trimmed) as string;
        if (typeof inner === 'string') {
          trimmed = inner.trim();
        }
      } catch (_e) {
        // ignore
      }
    }
    return trimmed;
  }

  private hasWsRawPrefix(value: string): boolean {
    return value.startsWith('__ws_raw__') || value.startsWith('_ws_raw_');
  }

  private stripWsRawPrefix(value: string): string {
    return value.replace(/^_+ws_raw_+/, '');
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

  private log(message: string): void {
    if (this.logger) {
      this.logger(message);
    }
  }

  private truncate(text: string, limit: number): string {
    if (text.length <= limit) {
      return text;
    }
    return `${text.slice(0, limit)}...`;
  }
}

class DeviceToolsEndpointListener implements McpWsClientListener {
  private owner: DeviceToolsEndpoint;

  constructor(owner: DeviceToolsEndpoint) {
    this.owner = owner;
  }

  onOpen(): void {
    this.owner.handleOpen();
  }

  onMessage(message: string): void {
    this.owner.handleMessage(message);
  }

  onClose(code: number, reason: string): void {
    this.owner.handleClose(code, reason);
  }

  onError(message: string): void {
    this.owner.handleError(message);
  }
}
