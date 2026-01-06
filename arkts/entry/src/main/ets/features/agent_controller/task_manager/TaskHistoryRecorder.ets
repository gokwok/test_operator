import type { AgentAction, ActionResult } from '../agentspace/AgentSpace';
import { TaskHistoryStore } from '../../persistence/history/TaskHistoryStore';
import type {
  HistoryStepBaseMeta,
  HistoryTaskMeta,
  HistoryTaskMetaPatch,
  HistoryTaskStatus
} from '../../persistence/history/TaskHistoryStore';

export class TaskHistoryRecorder {
  private store: TaskHistoryStore;

  constructor(store: TaskHistoryStore) {
    this.store = store;
  }

  createTaskId(): string {
    return this.store.createTaskId();
  }

  recordTaskStart(
    taskId: string,
    taskText: string,
    endpoint: string,
    sessionId: string,
    waitAfterActionMs: number
  ): void {
    if (taskId.length === 0) {
      return;
    }
    const meta: HistoryTaskMeta = {
      taskId,
      sessionId,
      taskText,
      cloudEndpoint: endpoint,
      waitAfterActionMs,
      status: 'running',
      startAtMs: Date.now(),
      endAtMs: 0,
      stepCount: 0,
      finishContent: '',
      error: '',
    };
    void this.store.createTask(meta);
  }

  updateSessionId(taskId: string, sessionId: string): void {
    if (taskId.length === 0) {
      return;
    }
    void this.store.updateTaskMeta(taskId, { sessionId });
  }

  recordStep(taskId: string, stepIndex: number, description: string, action: AgentAction, screenshot: string): void {
    if (taskId.length === 0 || stepIndex <= 0) {
      return;
    }
    const meta: HistoryStepBaseMeta = {
      taskId,
      stepIndex,
      tsMs: Date.now(),
      description,
      actionName: action.name,
      actionJson: JSON.stringify(action),
      resultStatus: '',
      error: '',
      output: '',
    };
    void this.store.putStep(taskId, meta, screenshot);
  }

  recordStepResult(taskId: string, stepIndex: number, result: ActionResult): void {
    if (taskId.length === 0 || stepIndex <= 0) {
      return;
    }
    const status = result.status;
    const error = result.error ?? '';
    const output = result.output ?? '';
    void this.store.updateStepResult(taskId, stepIndex, status, error, output);
  }

  finalizeTask(taskId: string, status: HistoryTaskStatus, detail: string): void {
    if (taskId.length === 0) {
      return;
    }
    const trimmed = detail.trim();
    const patch: HistoryTaskMetaPatch = {
      status,
      endAtMs: Date.now(),
      finishContent: status === 'finished' ? trimmed : '',
      error: status === 'finished' ? '' : trimmed,
    };
    void this.store.updateTaskMeta(taskId, patch);
  }
}
