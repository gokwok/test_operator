import distributedKVStore from '@ohos.data.distributedKVStore';
import type { common } from '@kit.AbilityKit';
import type { BusinessError } from '@kit.BasicServicesKit';
import image from '@ohos.multimedia.image';
import util from '@ohos.util';

const STORE_ID: string = 'agent_history';
const KEY_INDEX: string = 'H1:IDX';
const KEY_PREFIX_TASK: string = 'H1:T:';
const KEY_PREFIX_STEP: string = 'H1:S:';
const KEY_PREFIX_IMAGE: string = 'H1:I:';

const STEP_PAD_WIDTH: number = 6;
const CHUNK_PAD_WIDTH: number = 4;
const IMAGE_CHUNK_SIZE_BYTES: number = 512 * 1024;
const IMAGE_FORMAT: string = 'jpeg';

export type HistoryTaskStatus = 'running' | 'finished' | 'failed' | 'stopped';

export interface HistoryTaskMeta {
  taskId: string;
  sessionId: string;
  taskText: string;
  cloudEndpoint: string;
  waitAfterActionMs: number;
  status: HistoryTaskStatus;
  startAtMs: number;
  endAtMs: number;
  stepCount: number;
  finishContent: string;
  error: string;
}

export interface HistoryStepImageMeta {
  format: string;
  bytes: number;
  chunkSize: number;
  chunkCount: number;
}

export interface HistoryStepMeta {
  taskId: string;
  stepIndex: number;
  tsMs: number;
  description: string;
  actionName: string;
  actionJson: string;
  resultStatus: string;
  error: string;
  output: string;
  image: HistoryStepImageMeta;
}

export class TaskHistoryStore {
  private static instance: TaskHistoryStore | null = null;

  private ctx: common.Context | null = null;
  private manager: distributedKVStore.KVManager | null = null;
  private store: distributedKVStore.SingleKVStore | null = null;
  private storePromise: Promise<distributedKVStore.SingleKVStore> | null = null;
  private writeChain: Promise<void> = Promise.resolve();
  private enabled: boolean = true;
  private maxTasks: number = 50;

  static getInstance(): TaskHistoryStore {
    if (!TaskHistoryStore.instance) {
      TaskHistoryStore.instance = new TaskHistoryStore();
    }
    return TaskHistoryStore.instance;
  }

  init(ctx: common.Context): void {
    this.ctx = ctx;
  }

  configure(enabled: boolean, maxTasks: number): void {
    this.enabled = enabled;
    if (Number.isFinite(maxTasks)) {
      this.maxTasks = Math.max(0, Math.floor(maxTasks));
    }
  }

  createTaskId(): string {
    const rand = Math.floor(Math.random() * 1_000_000);
    return `${Date.now()}-${rand.toString().padStart(6, '0')}`;
  }

  async createTask(meta: HistoryTaskMeta): Promise<void> {
    if (!this.enabled) {
      return;
    }
    await this.enqueueWrite(async () => {
      const store = await this.getStore();
      await store.put(TaskHistoryStore.taskKey(meta.taskId), JSON.stringify(meta));
      await this.insertIndex(store, meta.taskId);
      await this.pruneIfNeeded(store);
    });
  }

  async updateTaskMeta(taskId: string, patch: Partial<HistoryTaskMeta>): Promise<void> {
    if (!this.enabled) {
      return;
    }
    await this.enqueueWrite(async () => {
      const store = await this.getStore();
      const current = await this.getTaskMeta(store, taskId);
      if (!current) {
        return;
      }
      const next: HistoryTaskMeta = { ...current, ...patch };
      await store.put(TaskHistoryStore.taskKey(taskId), JSON.stringify(next));
    });
  }

  async putStep(taskId: string, meta: Omit<HistoryStepMeta, 'image'>, screenshotBase64: string): Promise<void> {
    if (!this.enabled) {
      return;
    }
    const trimmed = screenshotBase64.trim();
    if (trimmed.length === 0) {
      return;
    }
    const helper = new util.Base64Helper();
    const bytes = helper.decodeSync(trimmed);
    const imageMeta = TaskHistoryStore.buildImageMeta(bytes.byteLength);
    const step: HistoryStepMeta = {
      ...meta,
      image: imageMeta,
    };

    await this.enqueueWrite(async () => {
      const store = await this.getStore();
      await this.writeImageChunks(store, taskId, meta.stepIndex, bytes, imageMeta.chunkSize);
      await store.put(TaskHistoryStore.stepKey(taskId, meta.stepIndex), JSON.stringify(step));
      const currentTask = await this.getTaskMeta(store, taskId);
      if (currentTask && meta.stepIndex > currentTask.stepCount) {
        await store.put(
          TaskHistoryStore.taskKey(taskId),
          JSON.stringify({ ...currentTask, stepCount: meta.stepIndex })
        );
      }
    });
  }

  async updateStepResult(taskId: string, stepIndex: number, status: string, error: string, output: string): Promise<void> {
    if (!this.enabled) {
      return;
    }
    await this.enqueueWrite(async () => {
      const store = await this.getStore();
      const key = TaskHistoryStore.stepKey(taskId, stepIndex);
      const raw = await this.safeGetString(store, key);
      if (raw.length === 0) {
        return;
      }
      const parsed = JSON.parse(raw) as HistoryStepMeta;
      const next: HistoryStepMeta = {
        ...parsed,
        resultStatus: status,
        error,
        output,
      };
      await store.put(key, JSON.stringify(next));
    });
  }

  async listTasks(limit: number): Promise<HistoryTaskMeta[]> {
    const store = await this.getStore();
    const ids = await this.readIndex(store);
    const result: HistoryTaskMeta[] = [];
    const capped = Number.isFinite(limit) ? Math.max(0, Math.floor(limit)) : 0;
    const count = capped > 0 ? Math.min(ids.length, capped) : ids.length;
    for (let i = 0; i < count; i++) {
      const meta = await this.getTaskMeta(store, ids[i]);
      if (meta) {
        result.push(meta);
      }
    }
    return result;
  }

  async getTask(taskId: string): Promise<HistoryTaskMeta | null> {
    const store = await this.getStore();
    return await this.getTaskMeta(store, taskId);
  }

  async listSteps(taskId: string): Promise<HistoryStepMeta[]> {
    const store = await this.getStore();
    const task = await this.getTaskMeta(store, taskId);
    if (!task || task.stepCount <= 0) {
      return [];
    }
    const steps: HistoryStepMeta[] = [];
    for (let i = 1; i <= task.stepCount; i++) {
      const raw = await this.safeGetString(store, TaskHistoryStore.stepKey(taskId, i));
      if (raw.length === 0) {
        continue;
      }
      try {
        const meta = JSON.parse(raw) as HistoryStepMeta;
        steps.push(meta);
      } catch {
        // ignore bad entry
      }
    }
    return steps;
  }

  async getStepPixelMap(taskId: string, stepIndex: number): Promise<image.PixelMap | null> {
    const bytes = await this.getStepImageBytes(taskId, stepIndex);
    if (!bytes) {
      return null;
    }
    let source: image.ImageSource | null = null;
    try {
      const buffer = bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength);
      source = image.createImageSource(buffer);
      return await source.createPixelMap();
    } catch {
      return null;
    } finally {
      if (source) {
        try {
          await source.release();
        } catch {
          // ignore
        }
      }
    }
  }

  async deleteTask(taskId: string): Promise<void> {
    await this.enqueueWrite(async () => {
      const store = await this.getStore();
      const task = await this.getTaskMeta(store, taskId);
      if (task) {
        for (let i = 1; i <= task.stepCount; i++) {
          await this.deleteStep(store, taskId, i);
        }
      }
      await store.delete(TaskHistoryStore.taskKey(taskId));
      const ids = await this.readIndex(store);
      const next = ids.filter(id => id !== taskId);
      await store.put(KEY_INDEX, JSON.stringify(next));
    });
  }

  private async deleteStep(store: distributedKVStore.SingleKVStore, taskId: string, stepIndex: number): Promise<void> {
    const stepKey = TaskHistoryStore.stepKey(taskId, stepIndex);
    const raw = await this.safeGetString(store, stepKey);
    if (raw.length > 0) {
      try {
        const parsed = JSON.parse(raw) as HistoryStepMeta;
        const chunkCount = parsed?.image?.chunkCount ?? 0;
        for (let i = 0; i < chunkCount; i++) {
          await store.delete(TaskHistoryStore.imageChunkKey(taskId, stepIndex, i));
        }
      } catch {
        // ignore
      }
    }
    await store.delete(stepKey);
  }

  private async getStepImageBytes(taskId: string, stepIndex: number): Promise<Uint8Array | null> {
    const store = await this.getStore();
    const raw = await this.safeGetString(store, TaskHistoryStore.stepKey(taskId, stepIndex));
    if (raw.length === 0) {
      return null;
    }
    let meta: HistoryStepMeta;
    try {
      meta = JSON.parse(raw) as HistoryStepMeta;
    } catch {
      return null;
    }
    const count = meta?.image?.chunkCount ?? 0;
    if (count <= 0) {
      return null;
    }
    const chunks: Uint8Array[] = [];
    let total = 0;
    for (let i = 0; i < count; i++) {
      const key = TaskHistoryStore.imageChunkKey(taskId, stepIndex, i);
      const value = await store.get(key) as Uint8Array;
      if (!value || value.byteLength === 0) {
        return null;
      }
      const copy = new Uint8Array(value);
      chunks.push(copy);
      total += copy.byteLength;
    }
    const merged = new Uint8Array(total);
    let offset = 0;
    for (let i = 0; i < chunks.length; i++) {
      merged.set(chunks[i], offset);
      offset += chunks[i].byteLength;
    }
    return merged;
  }

  private async writeImageChunks(
    store: distributedKVStore.SingleKVStore,
    taskId: string,
    stepIndex: number,
    bytes: Uint8Array,
    chunkSize: number
  ): Promise<void> {
    const size = Math.max(1, Math.floor(chunkSize));
    const total = bytes.byteLength;
    const count = Math.ceil(total / size);
    for (let i = 0; i < count; i++) {
      const start = i * size;
      const end = Math.min(total, start + size);
      const chunk = bytes.slice(start, end);
      await store.put(TaskHistoryStore.imageChunkKey(taskId, stepIndex, i), chunk);
    }
  }

  private async getStore(): Promise<distributedKVStore.SingleKVStore> {
    if (this.store) {
      return this.store;
    }
    if (this.storePromise) {
      return await this.storePromise;
    }
    const ctx = this.ctx;
    if (!ctx) {
      throw new Error('history_store_context_missing');
    }
    const config: distributedKVStore.KVManagerConfig = {
      context: ctx,
      bundleName: ctx.bundleName,
    };
    const options: distributedKVStore.Options = {
      createIfMissing: true,
      encrypt: false,
      backup: false,
      autoSync: false,
      kvStoreType: distributedKVStore.KVStoreType.SINGLE_VERSION,
    };
    const promise = distributedKVStore.createKVManager(config)
      .then((manager: distributedKVStore.KVManager) => {
        this.manager = manager;
        return manager.getKVStore<distributedKVStore.SingleKVStore>(STORE_ID, options);
      })
      .then((store: distributedKVStore.SingleKVStore) => {
        this.store = store;
        this.storePromise = null;
        return store;
      })
      .catch((e: BusinessError) => {
        this.storePromise = null;
        throw e;
      });
    this.storePromise = promise;
    return await promise;
  }

  private async getTaskMeta(store: distributedKVStore.SingleKVStore, taskId: string): Promise<HistoryTaskMeta | null> {
    const raw = await this.safeGetString(store, TaskHistoryStore.taskKey(taskId));
    if (raw.length === 0) {
      return null;
    }
    try {
      return JSON.parse(raw) as HistoryTaskMeta;
    } catch {
      return null;
    }
  }

  private async safeGetString(store: distributedKVStore.SingleKVStore, key: string): Promise<string> {
    try {
      const value = await store.get(key);
      if (typeof value === 'string') {
        return value;
      }
      return String(value ?? '');
    } catch {
      return '';
    }
  }

  private async enqueueWrite(task: () => Promise<void>): Promise<void> {
    const chained = this.writeChain.then(task).catch(() => {});
    this.writeChain = chained;
    await chained;
  }

  private async readIndex(store: distributedKVStore.SingleKVStore): Promise<string[]> {
    const raw = await this.safeGetString(store, KEY_INDEX);
    if (raw.length === 0) {
      return [];
    }
    try {
      const parsed = JSON.parse(raw) as string[];
      if (!Array.isArray(parsed)) {
        return [];
      }
      const result: string[] = [];
      for (let i = 0; i < parsed.length; i++) {
        const value = parsed[i];
        if (typeof value === 'string' && value.length > 0) {
          result.push(value);
        }
      }
      return result;
    } catch {
      return [];
    }
  }

  private async insertIndex(store: distributedKVStore.SingleKVStore, taskId: string): Promise<void> {
    const ids = await this.readIndex(store);
    const next = [taskId, ...ids.filter(id => id !== taskId)];
    await store.put(KEY_INDEX, JSON.stringify(next));
  }

  private async pruneIfNeeded(store: distributedKVStore.SingleKVStore): Promise<void> {
    if (this.maxTasks <= 0) {
      return;
    }
    const ids = await this.readIndex(store);
    if (ids.length <= this.maxTasks) {
      return;
    }
    const toRemove = ids.slice(this.maxTasks);
    const keep = ids.slice(0, this.maxTasks);
    await store.put(KEY_INDEX, JSON.stringify(keep));
    for (let i = 0; i < toRemove.length; i++) {
      const id = toRemove[i];
      const task = await this.getTaskMeta(store, id);
      if (task) {
        for (let s = 1; s <= task.stepCount; s++) {
          await this.deleteStep(store, id, s);
        }
      }
      await store.delete(TaskHistoryStore.taskKey(id));
    }
  }

  private static buildImageMeta(bytes: number): HistoryStepImageMeta {
    const chunkSize = IMAGE_CHUNK_SIZE_BYTES;
    const chunkCount = Math.ceil(bytes / chunkSize);
    return {
      format: IMAGE_FORMAT,
      bytes,
      chunkSize,
      chunkCount,
    };
  }

  private static taskKey(taskId: string): string {
    return `${KEY_PREFIX_TASK}${taskId}`;
  }

  private static stepKey(taskId: string, stepIndex: number): string {
    return `${KEY_PREFIX_STEP}${taskId}:${TaskHistoryStore.padNumber(stepIndex, STEP_PAD_WIDTH)}`;
  }

  private static imageChunkKey(taskId: string, stepIndex: number, chunkIndex: number): string {
    const step = TaskHistoryStore.padNumber(stepIndex, STEP_PAD_WIDTH);
    const chunk = TaskHistoryStore.padNumber(chunkIndex, CHUNK_PAD_WIDTH);
    return `${KEY_PREFIX_IMAGE}${taskId}:${step}:${chunk}`;
  }

  private static padNumber(value: number, width: number): string {
    const safe = Number.isFinite(value) ? Math.max(0, Math.floor(value)) : 0;
    return safe.toString().padStart(width, '0');
  }
}

