import type { TaskEvent, TaskEventBatch, TaskEventType } from '../types';

export class AgentEventHub {
  private eventSequence: number = 0;
  private events: Array<TaskEvent> = [];
  private readonly maxEvents: number = 200;

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
    if (this.events.length > this.maxEvents) {
      const overflow = this.events.length - this.maxEvents;
      if (overflow > 0) {
        this.events.splice(0, overflow);
      }
    }
  }

  pull(lastSequence: number): TaskEventBatch | null {
    if (this.events.length === 0 || this.eventSequence === lastSequence) {
      return null;
    }
    const filtered: Array<TaskEvent> = [];
    for (let i = 0; i < this.events.length; i += 1) {
      const event = this.events[i];
      if (event.seq > lastSequence) {
        filtered.push(event);
      }
    }
    if (filtered.length === 0) {
      return null;
    }
    const batch: TaskEventBatch = {
      sequence: this.eventSequence,
      events: filtered,
    };
    return batch;
  }
}
