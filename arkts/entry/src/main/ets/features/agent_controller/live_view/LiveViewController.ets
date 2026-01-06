import { notificationManager } from '@kit.NotificationKit';
import notificationSubscribe from '@ohos.notificationSubscribe';
import { wantAgent } from '@kit.AbilityKit';
import type { common, Want, WantAgent } from '@kit.AbilityKit';
import type { BusinessError } from '@kit.BasicServicesKit';
import image from '@ohos.multimedia.image';
import { AppState } from '../state_collector/AppState';

const LOG_TAG: string = '[LiveView]';
const LIVEVIEW_LOG_TAG: string = '[AS_LIVEVIEW]';
const DEFAULT_APP_NAME: string = 'com.huawei.aios.agent.operation';

export enum LiveViewType {
  VIEWER = 0,
}

export interface LiveViewPrimaryData {
  title: string;
  content: string;
}

export interface LiveViewCapsuleData {
  title: string;
}

export interface LiveViewTapTarget {
  bundleName: string;
  abilityName: string;
  action?: string;
}

export interface LiveViewContents {
  type: LiveViewType;
  id?: number;
  stopTimeoutMs?: number;
  typeCode?: number;
  primary: LiveViewPrimaryData;
  capsule: LiveViewCapsuleData;
  capsuleIcon?: image.PixelMap;
  tapTarget?: LiveViewTapTarget;
}

export type LiveViewCancelListener = (notificationId: number, reason?: number) => void;

export class LiveViewController {
  private static instance: LiveViewController | null = null;

  private subscribed: boolean = false;
  private cancelSubscribed: boolean = false;
  private cancelSubscriber: notificationSubscribe.NotificationSubscriber | null = null;
  private cancelListener: LiveViewCancelListener | null = null;
  private defaultIcon: image.PixelMap | null = null;
  private defaultIconInFlight: Promise<image.PixelMap | null> | null = null;

  static getInstance(): LiveViewController {
    if (!LiveViewController.instance) {
      LiveViewController.instance = new LiveViewController();
    }
    return LiveViewController.instance;
  }

  setCancelListener(listener?: LiveViewCancelListener): void {
    this.cancelListener = listener ?? null;
    if (this.cancelListener) {
      void this.ensureCancelSubscribed();
    }
  }

  async startDefault(ctx: common.Context, taskText: string): Promise<void> {
    const content = taskText.trim().length > 0 ? taskText.trim() : 'xxxxxxx';
    const icon = await this.resolveDefaultIcon(ctx);
    console.info(`${LIVEVIEW_LOG_TAG} controller startDefault contentLen=${content.length} icon=${icon ? 'yes' : 'no'}`);
    const params: LiveViewContents = {
      type: LiveViewType.VIEWER,
      primary: { title: '任务执行中', content },
      capsule: { title: '操作应用' },
      capsuleIcon: icon ?? undefined,
      tapTarget: LiveViewController.buildViewerTapTarget(),
    };
    await this.start(ctx, params);
  }

  async updateDefault(ctx: common.Context, taskText: string): Promise<void> {
    const content = taskText.trim().length > 0 ? taskText.trim() : 'xxxxxxx';
    const icon = await this.resolveDefaultIcon(ctx);
    console.info(`${LIVEVIEW_LOG_TAG} controller updateDefault contentLen=${content.length} icon=${icon ? 'yes' : 'no'}`);
    const params: LiveViewContents = {
      type: LiveViewType.VIEWER,
      primary: { title: '任务执行中', content },
      capsule: { title: '操作应用' },
      capsuleIcon: icon ?? undefined,
      tapTarget: LiveViewController.buildViewerTapTarget(),
    };
    await this.update(ctx, params);
  }

  async stopDefault(ctx: common.Context, timeoutMs?: number): Promise<void> {
    console.info(`${LIVEVIEW_LOG_TAG} controller stopDefault timeoutMs=${timeoutMs ?? -1}`);
    const params: LiveViewContents = {
      type: LiveViewType.VIEWER,
      stopTimeoutMs: timeoutMs,
      primary: { title: '', content: '' },
      capsule: { title: '' },
      tapTarget: LiveViewController.buildViewerTapTarget(),
    };
    await this.stop(ctx, params);
  }

  async start(ctx: common.Context, param: LiveViewContents): Promise<void> {
    console.info(`${LIVEVIEW_LOG_TAG} controller start title=${param.primary.title} capsule=${param.capsule.title} icon=${param.capsuleIcon ? 'yes' : 'no'}`);
    const request = await this.buildNotificationRequest(param);
    await this.ensureSubscribed();
    await this.publishOnce(request);
    void ctx;
  }

  async update(ctx: common.Context, param: LiveViewContents): Promise<void> {
    console.info(`${LIVEVIEW_LOG_TAG} controller update title=${param.primary.title} capsule=${param.capsule.title} icon=${param.capsuleIcon ? 'yes' : 'no'}`);
    const request = await this.buildNotificationRequest(param);
    await this.ensureSubscribed();
    await this.publishOnce(request);
    void ctx;
  }

  async stop(ctx: common.Context, param: LiveViewContents): Promise<void> {
    console.info(`${LIVEVIEW_LOG_TAG} controller stop title=${param.primary.title} capsule=${param.capsule.title} timeoutMs=${param.stopTimeoutMs ?? -1}`);
    const request = await this.buildNotificationRequest(param);
    const timeout = typeof param.stopTimeoutMs === 'number' ? param.stopTimeoutMs : 100;

    try {
      await this.publishOnce(request);
    } catch (e) {
      console.warn(`${LOG_TAG} stop publish failed: ${String(e)}`);
    }

    await new Promise<void>((resolve) => {
      setTimeout(() => {
        const cancelCallback = (err?: BusinessError): void => {
          if (err && err.code) {
            console.error(`${LOG_TAG} cancel failed. Code=${err.code}, message=${err.message}`);
          } else {
            console.info(`${LOG_TAG} stopped live view. id=${request.id}`);
          }
          resolve();
        };
        notificationManager.cancel(request.id, cancelCallback);
      }, timeout);
    });
    void ctx;
  }

  private static buildViewerTapTarget(): LiveViewTapTarget {
    const target: LiveViewTapTarget = {
      bundleName: 'com.huawei.aios.agent.operation',
      abilityName: 'AgentSpaceViewerAbility',
    };
    return target;
  }

  private async resolveDefaultIcon(ctx: common.Context): Promise<image.PixelMap | null> {
    if (this.defaultIcon) {
      return this.defaultIcon;
    }
    if (this.defaultIconInFlight) {
      return await this.defaultIconInFlight;
    }
    const task = this.loadDefaultAppIcon(ctx);
    this.defaultIconInFlight = task;
    try {
      this.defaultIcon = await task;
    } finally {
      this.defaultIconInFlight = null;
    }
    return this.defaultIcon;
  }

  private async loadDefaultAppIcon(ctx: common.Context): Promise<image.PixelMap | null> {
    try {
      const icon = await AppState.getIconByAppName(DEFAULT_APP_NAME, ctx);
      return icon;
    } catch (e) {
      console.warn(`${LOG_TAG} loadDefaultAppIcon failed: ${String(e)}`);
      return null;
    }
  }

  private async ensureSubscribed(): Promise<void> {
    if (this.subscribed) {
      return;
    }
    const subscriber: notificationManager.SystemLiveViewSubscriber = {};
    try {
      console.info(`${LIVEVIEW_LOG_TAG} subscribeSystemLiveView begin`);
      await notificationManager.subscribeSystemLiveView(subscriber);
      this.subscribed = true;
      console.info(`${LIVEVIEW_LOG_TAG} subscribeSystemLiveView ok`);
    } catch (e) {
      console.warn(`${LOG_TAG} subscribeSystemLiveView failed: ${String(e)}`);
    }
  }

  private async ensureCancelSubscribed(): Promise<void> {
    if (this.cancelSubscribed) {
      return;
    }
    const subscriber: notificationSubscribe.NotificationSubscriber = {
      onCancel: (data: notificationSubscribe.SubscribeCallbackData) => {
        this.handleCancel(data);
      },
      onBatchCancel: (batch: Array<notificationSubscribe.SubscribeCallbackData>) => {
        for (let i = 0; i < batch.length; i++) {
          this.handleCancel(batch[i]);
        }
      }
    };
    try {
      console.info(`${LIVEVIEW_LOG_TAG} subscribeSelf begin`);
      await notificationSubscribe.subscribeSelf(subscriber);
      this.cancelSubscriber = subscriber;
      this.cancelSubscribed = true;
      console.info(`${LIVEVIEW_LOG_TAG} subscribeSelf ok`);
    } catch (e) {
      console.warn(`${LOG_TAG} subscribeSelf failed: ${String(e)}`);
    }
  }

  private handleCancel(data: notificationSubscribe.SubscribeCallbackData): void {
    if (!this.cancelListener) {
      return;
    }
    const request = data.request;
    const id = request?.id;
    if (typeof id !== 'number') {
      return;
    }
    console.info(`${LIVEVIEW_LOG_TAG} cancel received id=${id} reason=${data.reason ?? -1}`);
    this.cancelListener(id, data.reason);
  }

  private async buildNotificationRequest(param: LiveViewContents): Promise<notificationManager.NotificationRequest> {
    const id = typeof param.id === 'number' ? param.id : LiveViewController.defaultNotificationId(param.type);
    const typeCode = typeof param.typeCode === 'number' ? param.typeCode : 3;
    const tapTarget = param.tapTarget ?? LiveViewController.buildViewerTapTarget();
    const wantAgentObj = await LiveViewController.buildWantAgent(tapTarget);
    const capsuleIcon = param.capsuleIcon;
    console.info(`${LIVEVIEW_LOG_TAG} build request id=${id} title=${param.primary.title} capsule=${param.capsule.title} icon=${capsuleIcon ? 'yes' : 'no'} textLen=${param.primary.content.length}`);

    const request: notificationManager.NotificationRequest = {
      notificationSlotType: notificationManager.SlotType.LIVE_VIEW,
      id,
      wantAgent: wantAgentObj,
      content: {
        notificationContentType: notificationManager.ContentType.NOTIFICATION_CONTENT_SYSTEM_LIVE_VIEW,
        systemLiveView: {
          title: param.primary.title,
          text: param.primary.content,
          typeCode,
          capsule: capsuleIcon ? { title: param.capsule.title, icon: capsuleIcon } : { title: param.capsule.title }
        }
      }
    };
    return request;
  }

  private static async buildWantAgent(target: LiveViewTapTarget): Promise<WantAgent> {
    const wantObj = LiveViewController.buildWant(target);
    const wantAgentInfo: wantAgent.WantAgentInfo = {
      wants: [wantObj],
      actionType: wantAgent.OperationType.START_ABILITIES,
      requestCode: 0,
      actionFlags: [wantAgent.WantAgentFlags.UPDATE_PRESENT_FLAG]
    };
    return await wantAgent.getWantAgent(wantAgentInfo);
  }

  private static buildWant(target: LiveViewTapTarget): Want {
    const wantObj: Want = {
      bundleName: target.bundleName,
      abilityName: target.abilityName,
    };
    if (typeof target.action === 'string' && target.action.length > 0) {
      wantObj.action = target.action;
    }
    return wantObj;
  }

  private async publishOnce(request: notificationManager.NotificationRequest): Promise<void> {
    console.info(`${LIVEVIEW_LOG_TAG} publish begin id=${request.id}`);
    await new Promise<void>((resolve, reject) => {
      const callback = (err?: BusinessError): void => {
        if (err && err.code) {
          const message = `${LOG_TAG} publish failed. Code=${err.code}, message=${err.message}, id=${request.id}`;
          console.error(message);
          console.error(`${LIVEVIEW_LOG_TAG} publish failed code=${err.code} message=${err.message} id=${request.id}`);
          reject(new Error(message));
          return;
        }
        console.info(`${LOG_TAG} publish ok. id=${request.id}`);
        console.info(`${LIVEVIEW_LOG_TAG} publish ok id=${request.id}`);
        resolve();
      };
      notificationManager.publish(request, callback);
    });
  }

  private static defaultNotificationId(type: number): number {
    return 10_000 + type;
  }
}
