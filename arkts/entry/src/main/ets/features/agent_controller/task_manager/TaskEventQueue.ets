import type { TaskEvent, TaskEventBatch, TaskEventType } from '../types';

export class TaskEventQueue {
  private eventSequence: number = 0;
  private events: Array<TaskEvent> = [];

  reset(): void {
    this.eventSequence = 0;
    this.events = [];
  }

  push(type: TaskEventType, message: string, payloadJson?: string): void {
    this.eventSequence += 1;
    const event: TaskEvent = {
      seq: this.eventSequence,
      type,
      message,
      payloadJson: payloadJson ?? '',
    };
    this.events.push(event);
  }

  pull(lastSequence: number): TaskEventBatch | null {
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
}
