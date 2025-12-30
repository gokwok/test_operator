import http from '@ohos.net.http';

import type { AgentAction, ActionParameters, ActionResult } from '../agentspace/AgentSpace';

export interface CloudDeviceState {
  screenshot: string;
  user_note?: string;
  installed_apps?: Array<string>;
}

export interface CloudInvokePayload {
  task?: string;
  state: CloudDeviceState;
  action_result?: ActionResult;
  control?: CloudControl;
}

export interface CloudControl {
  op: string;
  reason?: string;
}

export interface CloudInvokeRequest {
  session_id?: string;
  payload: CloudInvokePayload;
}

export interface CloudActionParameters {
  point?: Array<number>;
  start?: Array<number>;
  end?: Array<number>;
  start_point?: Array<number>;
  end_point?: Array<number>;
  duration_ms?: number;
  seconds?: number;
  content?: string;
  app?: string;
  options?: Array<string>;
}

export interface CloudAction {
  name?: string;
  parameters?: CloudActionParameters;
}

export interface CloudInvokeOutput {
  result?: string;
  session_id?: string;
  description?: string;
  action?: CloudAction;
}

export interface CloudInvokeResponse {
  spec_version?: string;
  session_id?: string;
  run_id?: string;
  output?: CloudInvokeOutput;
}

export class CloudInvokeError extends Error {
  statusCode: number;

  constructor(statusCode: number, message: string) {
    super(message);
    this.statusCode = statusCode;
  }
}

export class CloudAgentClient {
  private endpoint: string;

  constructor(endpoint: string) {
    this.endpoint = endpoint;
  }

  setEndpoint(endpoint: string): void {
    this.endpoint = endpoint.trim();
  }

  getEndpoint(): string {
    return this.endpoint;
  }

  async invoke(sessionId: string, payload: CloudInvokePayload): Promise<CloudInvokeResponse> {
    const request: CloudInvokeRequest = { payload };
    const session = sessionId.trim();
    if (session.length > 0) {
      request.session_id = session;
    }
    const url = this.buildInvokeUrl();
    const body = JSON.stringify(request);
    const client = http.createHttp();
    try {
      const response = await client.request(url, {
        method: http.RequestMethod.POST,
        header: { 'Content-Type': 'application/json' },
        extraData: body,
        connectTimeout: 10000,
        readTimeout: 60000,
      });
      const status = response.responseCode;
      const raw = this.readResponseText(response.result);
      if (status !== 200) {
        throw new CloudInvokeError(status, raw.length > 0 ? raw : `http_${status}`);
      }
      if (raw.length === 0) {
        throw new Error('empty_response');
      }
      const parsed = JSON.parse(raw) as CloudInvokeResponse;
      return parsed;
    } finally {
      client.destroy();
    }
  }

  normalizeAction(action: CloudAction): AgentAction | null {
    const name = (action.name ?? '').trim();
    if (name.length === 0) {
      return null;
    }
    const params = action.parameters;
    if (!params) {
      const plain: AgentAction = { name };
      return plain;
    }
    const normalized: ActionParameters = {};
    if (params.point && params.point.length >= 2) {
      normalized.point = params.point;
    }
    if (params.start_point && params.start_point.length >= 2) {
      normalized.start_point = params.start_point;
    }
    if (params.end_point && params.end_point.length >= 2) {
      normalized.end_point = params.end_point;
    }
    if (params.start && params.start.length >= 2) {
      normalized.start_point = params.start;
    }
    if (params.end && params.end.length >= 2) {
      normalized.end_point = params.end;
    }
    if (typeof params.duration_ms === 'number') {
      normalized.duration_ms = params.duration_ms;
    }
    if (typeof params.seconds === 'number') {
      normalized.seconds = params.seconds;
    }
    if (typeof params.content === 'string') {
      normalized.content = params.content;
    }
    if (typeof params.app === 'string') {
      normalized.app = params.app;
    }
    if (params.options && params.options.length > 0) {
      normalized.options = params.options;
    }
    const resolved: AgentAction = { name, parameters: normalized };
    return resolved;
  }

  private buildInvokeUrl(): string {
    const base = this.endpoint.trim();
    if (base.length === 0) {
      return '/invoke';
    }
    if (base.endsWith('/')) {
      return `${base}invoke`;
    }
    return `${base}/invoke`;
  }

  private readResponseText(result: string | Object): string {
    if (typeof result === 'string') {
      return result;
    }
    return String(result);
  }
}
