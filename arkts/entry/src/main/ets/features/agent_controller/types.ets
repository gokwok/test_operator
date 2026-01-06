export type TaskStatus = 'idle' | 'running' | 'waiting_user' | 'stopping' | 'finished' | 'failed';

export type ActionResultStatus = '' | 'SUCCESS' | 'FAILED' | 'SKIPPED';

export type TaskEventType = 'status' | 'log' | 'action' | 'result' | 'need_user' | 'error';

export interface TaskEvent {
  seq: number;
  type: TaskEventType;
  message: string;
  payloadJson: string;
}

export interface TaskEventBatch {
  sequence: number;
  events: Array<TaskEvent>;
}

export interface TaskSnapshot {
  sessionId: string;
  status: TaskStatus;
  stepIndex: number;
  lastDescription: string;
  lastActionName: string;
  lastActionJson: string;
  lastResultStatus: ActionResultStatus;
  lastError: string;
  installedAppsSent: boolean;
  appLaunched?: boolean;
}

export interface DeviceState {
  screenshot: string;
  user_note?: string;
  installed_apps?: Array<string>;
}

export interface AppTargetDescriptor {
  displayName: string;
  bundleName: string;
  abilityName: string;
}

export interface UserReply {
  kind: 'call_user' | 'interact' | 'take_over' | 'task_over';
  content: string;
}

export interface TaskStartRequest {
  sessionId: string;
  taskText: string;
  installedApps: Array<string>;
  appTargets: Array<AppTargetDescriptor>;
  waitMs: number;
}
