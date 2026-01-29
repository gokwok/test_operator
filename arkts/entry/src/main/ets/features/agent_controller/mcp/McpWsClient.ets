import webSocket from '@ohos.net.webSocket';

export interface McpWsClientListener {
  onOpen(): void;
  onMessage(message: string): void;
  onClose(code: number, reason: string): void;
  onError(message: string): void;
}

interface WsMessageValue {
  data?: Object;
  message?: Object;
  code?: number;
  reason?: string;
}

export class McpWsClient {
  private socket: webSocket.WebSocket | null = null;
  private listener: McpWsClientListener | null = null;
  private connected: boolean = false;
  private connecting: boolean = false;
  private url: string = '';

  setListener(listener: McpWsClientListener | null): void {
    this.listener = listener;
  }

  getUrl(): string {
    return this.url;
  }

  isConnected(): boolean {
    return this.connected;
  }

  isConnecting(): boolean {
    return this.connecting;
  }

  async connect(url: string): Promise<void> {
    if (this.connected || this.connecting) {
      return;
    }
    this.url = url.trim();
    if (this.url.length === 0) {
      throw new Error('ws_url_empty');
    }
    const socket = webSocket.createWebSocket();
    this.socket = socket;
    this.connecting = true;
    this.bindSocket(socket);
    await this.connectSocket(socket, this.url);
  }

  async disconnect(): Promise<void> {
    const socket = this.socket;
    if (!socket) {
      this.connected = false;
      this.connecting = false;
      return;
    }
    await this.closeSocket(socket);
    this.socket = null;
    this.connected = false;
    this.connecting = false;
  }

  async sendText(text: string): Promise<void> {
    const socket = this.socket;
    if (!socket || !this.connected) {
      throw new Error('ws_not_connected');
    }
    await this.sendSocket(socket, text);
  }

  private bindSocket(socket: webSocket.WebSocket): void {
    socket.on('open', (err: Object, _value: Object) => {
      const error = this.readError(err);
      if (error.length > 0) {
        this.connecting = false;
        this.connected = false;
        this.notifyError(error);
        return;
      }
      this.connecting = false;
      this.connected = true;
      this.notifyOpen();
    });

    socket.on('message', (err: Object, value: Object) => {
      const error = this.readError(err);
      if (error.length > 0) {
        this.notifyError(error);
        return;
      }
      const message = this.readMessage(value);
      if (message.length > 0) {
        this.notifyMessage(message);
      }
    });

    socket.on('close', (err: Object, value: Object) => {
      const error = this.readError(err);
      if (error.length > 0) {
        this.notifyError(error);
      }
      this.connected = false;
      this.connecting = false;
      const payload = value as WsMessageValue;
      const code = typeof payload.code === 'number' ? payload.code : 0;
      const reason = typeof payload.reason === 'string' ? payload.reason : '';
      this.notifyClose(code, reason);
    });

    socket.on('error', (err: Object) => {
      const error = this.readError(err);
      this.connected = false;
      this.connecting = false;
      if (error.length > 0) {
        this.notifyError(error);
      }
    });
  }

  private connectSocket(socket: webSocket.WebSocket, url: string): Promise<void> {
    return new Promise((resolve, reject) => {
      try {
        socket.connect(url, (err: Object, _value: Object) => {
          const error = this.readError(err);
          if (error.length > 0) {
            this.connecting = false;
            this.connected = false;
            reject(new Error(error));
            return;
          }
          resolve();
        });
      } catch (e) {
        this.connecting = false;
        this.connected = false;
        reject(new Error(String(e)));
      }
    });
  }

  private closeSocket(socket: webSocket.WebSocket): Promise<void> {
    return new Promise((resolve, reject) => {
      try {
        socket.close((err: Object, _value: Object) => {
          const error = this.readError(err);
          if (error.length > 0) {
            reject(new Error(error));
            return;
          }
          resolve();
        });
      } catch (e) {
        reject(new Error(String(e)));
      }
    });
  }

  private sendSocket(socket: webSocket.WebSocket, text: string): Promise<void> {
    return new Promise((resolve, reject) => {
      try {
        socket.send(text, (err: Object, _value: Object) => {
          const error = this.readError(err);
          if (error.length > 0) {
            reject(new Error(error));
            return;
          }
          resolve();
        });
      } catch (e) {
        reject(new Error(String(e)));
      }
    });
  }

  private readMessage(value: Object): string {
    const payload = value as WsMessageValue;
    const data = payload.data ?? payload.message;
    if (typeof data === 'string') {
      return data;
    }
    if (data instanceof ArrayBuffer) {
      const buffer = data as ArrayBuffer;
      return this.decodeArrayBuffer(buffer);
    }
    if (data instanceof Uint8Array) {
      return this.decodeUint8Array(data);
    }
    if (data) {
      return String(data);
    }
    return '';
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

  private readError(err: Object): string {
    if (!err) {
      return '';
    }
    const text = String(err);
    return text === '[object Object]' ? '' : text;
  }

  private notifyOpen(): void {
    if (this.listener) {
      this.listener.onOpen();
    }
  }

  private notifyMessage(message: string): void {
    if (this.listener) {
      this.listener.onMessage(message);
    }
  }

  private notifyClose(code: number, reason: string): void {
    if (this.listener) {
      this.listener.onClose(code, reason);
    }
  }

  private notifyError(message: string): void {
    if (this.listener) {
      this.listener.onError(message);
    }
  }
}
