export type TaskStatus = 'idle' | 'running' | 'waiting_user' | 'stopping' | 'finished' | 'failed';

export type ActionResultStatus = '' | 'SUCCESS' | 'FAILED' | 'SKIPPED';

export interface TaskSnapshot {
  sessionId: string;
  status: TaskStatus;
  stepIndex: number;
  lastDescription: string;
  lastActionName: string;
  lastResultStatus: ActionResultStatus;
  lastError: string;
}
