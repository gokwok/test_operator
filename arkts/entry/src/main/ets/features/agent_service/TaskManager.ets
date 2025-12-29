import type { ActionResultStatus, TaskEvent, TaskEventBatch, TaskEventType, TaskSnapshot } from './types';

export class TaskManager {
  private snapshot: TaskSnapshot = {
    sessionId: '',
    status: 'idle',
    stepIndex: 0,
    lastDescription: '',
    lastActionName: '',
    lastResultStatus: '',
    lastError: '',
  };
  private eventSequence: number = 0;
  private events: Array<TaskEvent> = [];
  private tickTimerId: number = -1;

  startFakeTask(sessionId?: string): void {
    const id = sessionId && sessionId.length > 0 ? sessionId : `s-${Date.now()}`;
    this.stopTimer();
    this.snapshot = {
      sessionId: id,
      status: 'running',
      stepIndex: 0,
      lastDescription: 'start',
      lastActionName: 'tick',
      lastResultStatus: '',
      lastError: '',
    };
    this.pushEvent('status', 'running');
    this.startTimer();
  }

  stopTask(reason: string): void {
    const status = this.snapshot.status;
    if (status === 'idle') {
      return;
    }
    this.stopTimer();
    this.snapshot.status = 'idle';
    this.snapshot.lastDescription = 'stopped';
    this.snapshot.lastError = reason;
    this.pushEvent('status', `stopped:${reason}`);
  }

  getSnapshot(): TaskSnapshot {
    const copy: TaskSnapshot = {
      sessionId: this.snapshot.sessionId,
      status: this.snapshot.status,
      stepIndex: this.snapshot.stepIndex,
      lastDescription: this.snapshot.lastDescription,
      lastActionName: this.snapshot.lastActionName,
      lastResultStatus: this.snapshot.lastResultStatus,
      lastError: this.snapshot.lastError,
    };
    return copy;
  }

  pullEvents(lastSequence: number): TaskEventBatch | null {
    if (this.events.length === 0 || this.eventSequence === lastSequence) {
      return null;
    }
    const batch: TaskEventBatch = {
      sequence: this.eventSequence,
      events: this.events,
    };
    this.events = [];
    return batch;
  }

  private startTimer(): void {
    if (this.tickTimerId !== -1) {
      return;
    }
    this.tickTimerId = setInterval(() => {
      this.onTick();
    }, 800);
  }

  private stopTimer(): void {
    if (this.tickTimerId === -1) {
      return;
    }
    clearInterval(this.tickTimerId);
    this.tickTimerId = -1;
  }

  private onTick(): void {
    if (this.snapshot.status !== 'running') {
      return;
    }
    this.snapshot.stepIndex += 1;
    this.snapshot.lastDescription = `step:${this.snapshot.stepIndex}`;
    this.snapshot.lastActionName = 'tick';
    this.snapshot.lastResultStatus = this.pickResultStatus(this.snapshot.stepIndex);
    this.pushEvent('log', this.snapshot.lastDescription);
    if (this.snapshot.stepIndex >= 5) {
      this.snapshot.status = 'finished';
      this.pushEvent('status', 'finished');
      this.stopTimer();
    }
  }

  private pickResultStatus(step: number): ActionResultStatus {
    if (step % 2 === 0) {
      return 'SUCCESS';
    }
    return 'SKIPPED';
  }

  private pushEvent(type: TaskEventType, message: string, payloadJson?: string): void {
    this.eventSequence += 1;
    const event: TaskEvent = {
      seq: this.eventSequence,
      type,
      message,
      payloadJson: payloadJson ?? '',
    };
    this.events.push(event);
  }
}
