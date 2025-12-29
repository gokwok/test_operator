export const AGENT_SERVICE_DESCRIPTOR: string = 'com.huawei.aios.agent.operation.AgentService';

export enum AgentServiceRequestCode {
  PING = 1,
  GET_SNAPSHOT = 2,
  START_FAKE_TASK = 3,
  STOP_TASK = 4,
  PULL_EVENTS = 5,
}
