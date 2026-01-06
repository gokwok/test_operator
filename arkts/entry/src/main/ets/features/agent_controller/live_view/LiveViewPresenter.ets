import image from '@ohos.multimedia.image';
import type { AgentAction } from '../agentspace/AgentSpace';
import type { TaskSnapshot } from '../types';
import { LiveViewContents, LiveViewType } from './LiveViewController';

const DEFAULT_PLACEHOLDER: string = '正在分析用户需求...';
const DEFAULT_FINISH_TEXT: string = '任务已完成';
const TITLE_RUNNING: string = '任务执行中';
const TITLE_WAITING: string = '等待答复';
const TITLE_FINISHED: string = '任务完成';
const TITLE_STOPPED: string = '任务已停止';
const TITLE_FAILED: string = '任务异常';
const CAPSULE_RUNNING: string = '操作应用';
const CAPSULE_INTERACTION: string = '需要交互';
const CAPSULE_FINISHED: string = '任务完成';
const CAPSULE_STOPPED: string = '任务已停止';
const CAPSULE_FAILED: string = '任务异常';

export interface LiveViewPresenterContext {
  notificationId: number;
  taskText: string;
  icon: image.PixelMap | null;
}

export class LiveViewPresenter {
  static buildRunning(ctx: LiveViewPresenterContext): LiveViewContents {
    const content = LiveViewPresenter.normalizeText(ctx.taskText, DEFAULT_PLACEHOLDER);
    return LiveViewPresenter.buildPayload(ctx, TITLE_RUNNING, content, CAPSULE_RUNNING);
  }

  static buildInteraction(ctx: LiveViewPresenterContext, content: string): LiveViewContents {
    const body = LiveViewPresenter.normalizeText(content, DEFAULT_PLACEHOLDER);
    return LiveViewPresenter.buildPayload(ctx, TITLE_WAITING, body, CAPSULE_INTERACTION);
  }

  static buildFinished(ctx: LiveViewPresenterContext, content: string): LiveViewContents {
    const body = LiveViewPresenter.normalizeText(content, DEFAULT_FINISH_TEXT);
    return LiveViewPresenter.buildPayload(ctx, TITLE_FINISHED, body, CAPSULE_FINISHED);
  }

  static buildStopped(ctx: LiveViewPresenterContext, reason: string): LiveViewContents {
    const body = LiveViewPresenter.normalizeStopReason(reason);
    return LiveViewPresenter.buildPayload(ctx, TITLE_STOPPED, body, CAPSULE_STOPPED);
  }

  static buildFailed(ctx: LiveViewPresenterContext, reason: string): LiveViewContents {
    const body = LiveViewPresenter.normalizeFailureReason(reason);
    return LiveViewPresenter.buildPayload(ctx, TITLE_FAILED, body, CAPSULE_FAILED);
  }

  static resolveActionContent(action: AgentAction, fallback: string): string {
    const content = action.parameters?.content;
    if (typeof content === 'string' && content.trim().length > 0) {
      return content.trim();
    }
    return LiveViewPresenter.normalizeText(fallback, DEFAULT_PLACEHOLDER);
  }

  static resolveInteractionContent(snapshot: TaskSnapshot): string {
    const json = snapshot.lastActionJson.trim();
    if (json.length > 0) {
      try {
        const parsed = JSON.parse(json) as AgentAction;
        return LiveViewPresenter.resolveActionContent(parsed, snapshot.lastDescription);
      } catch (_e) {
        // ignore
      }
    }
    return LiveViewPresenter.normalizeText(snapshot.lastDescription, DEFAULT_PLACEHOLDER);
  }

  static resolveFinishContent(snapshot: TaskSnapshot): string {
    const actionName = snapshot.lastActionName.trim().toLowerCase();
    if (actionName === 'finish') {
      const json = snapshot.lastActionJson.trim();
      if (json.length > 0) {
        try {
          const parsed = JSON.parse(json) as AgentAction;
          const content = parsed.parameters?.content;
          if (typeof content === 'string' && content.trim().length > 0) {
            return content.trim();
          }
        } catch (_e) {
          // ignore
        }
      }
    }
    const fallback = snapshot.lastDescription.trim();
    return fallback.length > 0 ? fallback : DEFAULT_FINISH_TEXT;
  }

  static normalizeStopReason(reason: string): string {
    const trimmed = reason.trim();
    if (trimmed.length === 0) {
      return TITLE_STOPPED;
    }
    if (trimmed === 'user_stop') {
      return '用户已停止任务';
    }
    if (trimmed === 'liveview_closed') {
      return '用户关闭了任务实况';
    }
    if (trimmed === 'task_over') {
      return '任务已结束';
    }
    return trimmed;
  }

  static normalizeFailureReason(reason: string): string {
    const trimmed = reason.trim();
    if (trimmed.length === 0) {
      return '任务执行失败';
    }
    if (trimmed === 'dependencies_missing') {
      return '运行环境未准备就绪';
    }
    if (trimmed === 'missing_action' || trimmed === 'empty_action') {
      return '未收到有效指令';
    }
    if (trimmed === 'task_text_empty') {
      return '任务内容为空';
    }
    if (trimmed.startsWith('invoke_failed:')) {
      return '任务请求失败，请稍后重试';
    }
    if (trimmed.startsWith('action_failed:')) {
      return '任务操作失败，请稍后重试';
    }
    return trimmed;
  }

  private static normalizeText(input: string, fallback: string): string {
    const trimmed = input.trim();
    return trimmed.length > 0 ? trimmed : fallback;
  }

  private static buildPayload(
    ctx: LiveViewPresenterContext,
    title: string,
    content: string,
    capsuleTitle: string
  ): LiveViewContents {
    const payload: LiveViewContents = {
      type: LiveViewType.VIEWER,
      id: ctx.notificationId > 0 ? ctx.notificationId : undefined,
      primary: { title, content },
      capsule: { title: capsuleTitle },
    };
    if (ctx.icon) {
      payload.capsuleIcon = ctx.icon;
    }
    return payload;
  }
}
