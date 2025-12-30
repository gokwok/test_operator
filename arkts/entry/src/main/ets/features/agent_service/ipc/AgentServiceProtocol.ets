export const AGENT_SERVICE_DESCRIPTOR: string = 'com.huawei.aios.agent.operation.AgentService';

export enum AgentServiceRequestCode {
  PING = 1,
  GET_SNAPSHOT = 2,
  START_FAKE_TASK = 3,
  STOP_TASK = 4,
  PULL_EVENTS = 5,
  ENSURE_VTS = 10,
  GET_RUNTIME = 11,
  LAUNCH_APP = 12,
  CLICK_NORM = 13,
  SCREENSHOT = 14,
  DESTROY_VTS = 15,
  STOP_VTS = 16,
}
