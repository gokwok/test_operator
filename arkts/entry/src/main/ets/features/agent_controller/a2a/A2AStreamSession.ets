import http from '@ohos.net.http';
import type { A2ARequest } from './A2AClient';

export interface A2AStreamListener {
  onEvent(payload: Object): void;
  onError(message: string): void;
  onComplete(): void;
}

class SseLineParser {
  private buffer: string = '';
  private onData: (data: string) => void;

  constructor(onData: (data: string) => void) {
    this.onData = onData;
  }

  reset(): void {
    this.buffer = '';
  }

  feed(chunk: string): void {
    if (chunk.length === 0) {
      return;
    }
    this.buffer += chunk;
    while (true) {
      const idx = this.buffer.indexOf('\n');
      if (idx < 0) {
        break;
      }
      let line = this.buffer.slice(0, idx);
      this.buffer = this.buffer.slice(idx + 1);
      if (line.endsWith('\r')) {
        line = line.slice(0, line.length - 1);
      }
      if (line.length === 0) {
        continue;
      }
      if (line.startsWith(':')) {
        continue;
      }
      if (!line.startsWith('data:')) {
        continue;
      }
      const data = line.slice(5).trim();
      if (data.length > 0) {
        this.onData(data);
      }
    }
  }
}

export class A2AStreamSession {
  private client: http.HttpRequest | null = null;
  private listener: A2AStreamListener | null = null;
  private parser: SseLineParser;
  private active: boolean = false;
  private parsedAny: boolean = false;

  constructor() {
    this.parser = new SseLineParser((data: string) => {
      this.handleData(data);
    });
  }

  isActive(): boolean {
    return this.active;
  }

  start(baseUrl: string, request: A2ARequest, listener: A2AStreamListener): void {
    if (this.active) {
      throw new Error('stream_active');
    }
    const url = this.buildMessageStreamUrl(baseUrl);
    if (url.length === 0) {
      throw new Error('a2a_base_url_empty');
    }
    const client = http.createHttp();
    this.client = client;
    this.listener = listener;
    this.active = true;
    this.parsedAny = false;
    this.parser.reset();

    client.on('dataReceive', (data: Object) => {
      const text = this.decodeChunk(data);
      if (text.length > 0) {
        this.parser.feed(text);
      }
    });

    const body = JSON.stringify(request);
    client.request(url, {
      method: http.RequestMethod.POST,
      header: {
        'Content-Type': 'application/json',
        'Accept': 'text/event-stream',
      },
      extraData: body,
      connectTimeout: 10000,
      readTimeout: 600000,
    }).then((response: http.HttpResponse) => {
      if (!this.parsedAny) {
        this.handleNonSseResponse(response);
      }
      this.active = false;
      if (this.listener) {
        this.listener.onComplete();
      }
    }).catch((err: Error) => {
      this.active = false;
      if (this.listener) {
        this.listener.onError(String(err));
      }
    });
  }

  stop(): void {
    const client = this.client;
    this.client = null;
    this.active = false;
    if (client) {
      try {
        client.destroy();
      } catch (_e) {
        // ignore
      }
    }
    if (this.listener) {
      this.listener.onComplete();
    }
  }

  private handleData(data: string): void {
    if (!this.listener) {
      return;
    }
    try {
      this.parsedAny = true;
      const parsed = JSON.parse(data) as Object;
      if (parsed && typeof parsed === 'object') {
        this.listener.onEvent(parsed);
      }
    } catch (e) {
      this.listener.onError(`invalid_sse_json:${String(e)}`);
    }
  }

  private handleNonSseResponse(response: http.HttpResponse): void {
    const status = response.responseCode;
    const raw = this.decodeChunk(response.result as Object);
    if (raw.length > 0) {
      this.parser.feed(raw);
      if (this.parsedAny) {
        return;
      }
      const snippet = raw.length > 400 ? raw.slice(0, 400) : raw;
      this.emitError(`non_sse_response:${status}:${snippet}`);
      return;
    }
    if (status !== 200) {
      this.emitError(`http_${status}`);
    }
  }

  private emitError(message: string): void {
    if (this.listener) {
      this.listener.onError(message);
    }
  }

  private decodeChunk(data: Object): string {
    if (typeof data === 'string') {
      return data;
    }
    if (data !== null && typeof data === 'object') {
      const obj = data as Object;
      if (obj instanceof ArrayBuffer) {
        return this.decodeArrayBuffer(obj as ArrayBuffer);
      }
      if (obj instanceof Uint8Array) {
        return this.decodeUint8Array(obj as Uint8Array);
      }
    }
    return String(data);
  }

  private decodeArrayBuffer(buffer: ArrayBuffer): string {
    const bytes = new Uint8Array(buffer);
    return this.decodeUint8Array(bytes);
  }

  private decodeUint8Array(bytes: Uint8Array): string {
    if (bytes.length === 0) {
      return '';
    }
    const chunkSize = 8192;
    let result = '';
    for (let offset = 0; offset < bytes.length; offset += chunkSize) {
      const end = Math.min(bytes.length, offset + chunkSize);
      const slice = bytes.subarray(offset, end);
      result += String.fromCharCode(...slice);
    }
    return result;
  }

  private buildMessageStreamUrl(baseUrl: string): string {
    const base = baseUrl.trim();
    if (base.length === 0) {
      return '';
    }
    const normalized = base.endsWith('/') ? base.slice(0, -1) : base;
    return `${normalized}/a2a/v1/message:stream`;
  }
}
