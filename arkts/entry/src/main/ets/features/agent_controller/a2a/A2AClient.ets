import http from '@ohos.net.http';

export interface A2AFile {
  name: string;
  mimeType: string;
  bytes: string;
}

export interface A2AFilePart {
  kind: string;
  file: A2AFile;
}

export interface A2ATextPart {
  kind: string;
  text: string;
}

export type A2AMessagePart = A2AFilePart | A2ATextPart;

export interface A2AMessage {
  messageId: string;
  role: string;
  contextId: string;
  taskId?: string;
  parts: Array<A2AMessagePart>;
}

export interface A2AConfiguration {
  blocking: boolean;
}

export interface A2AMetadata {
  installed_apps?: Array<string>;
}

export interface A2ARequest {
  message: A2AMessage;
  configuration: A2AConfiguration;
  metadata?: A2AMetadata;
}

export class A2AClient {
  private baseUrl: string;

  constructor(baseUrl: string) {
    this.baseUrl = baseUrl.trim();
  }

  setBaseUrl(baseUrl: string): void {
    this.baseUrl = baseUrl.trim();
  }

  async messageSend(request: A2ARequest): Promise<string> {
    const url = this.buildMessageSendUrl();
    if (url.length === 0) {
      throw new Error('a2a_base_url_empty');
    }
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
        throw new Error(`http_${status}:${raw}`);
      }
      return raw;
    } finally {
      client.destroy();
    }
  }

  private buildMessageSendUrl(): string {
    const base = this.baseUrl.trim();
    if (base.length === 0) {
      return '';
    }
    const normalized = base.endsWith('/') ? base.slice(0, -1) : base;
    return `${normalized}/a2a/v1/message:send`;
  }

  private readResponseText(result: string | Object): string {
    if (typeof result === 'string') {
      return result;
    }
    return String(result);
  }
}
