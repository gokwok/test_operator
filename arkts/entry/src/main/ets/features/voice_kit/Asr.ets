import { abilityAccessCtrl, common, Permissions } from '@kit.AbilityKit';
import { speechRecognizer } from '@kit.CoreSpeechKit';
import type { BusinessError } from '@kit.BasicServicesKit';
import { promptAction } from '@kit.ArkUI';

export interface AsrCallbacks {
  onStart?: () => void;
  onPartialResult?: (text: string) => void;
  onFinalResult?: (text: string) => void;
  onComplete?: () => void;
  onError?: (code: number, message: string) => void;
}

interface SpeechRecognizerEngineExtraParams extends Record<string, Object> {
  locate: string;
  recognizerMode: string;
}

interface SpeechRecognizerStartExtraParams extends Record<string, Object> {
  recognitionMode: number;
  vadBegin: number;
  vadEnd: number;
  maxAudioDuration: number;
}

class AsrFeature {
  private speechEngine?: speechRecognizer.SpeechRecognitionEngine;
  private engineCreatePromise?: Promise<speechRecognizer.SpeechRecognitionEngine>;
  private activeSessionId: string = '';
  private activeCallbacks?: AsrCallbacks;

  startTest(context: common.UIAbilityContext): void {
    void this.startListening(context, {
      onStart: () => {
        promptAction.showToast({ message: 'ASR started', duration: 2000 });
      },
      onFinalResult: (text: string) => {
        promptAction.showToast({ message: `ASR: ${text}`, duration: 2000 });
      },
      onError: () => {
        promptAction.showToast({ message: 'ASR start failed', duration: 2000 });
      }
    });
  }

  finish(): void {
    this.stopListening(false);
  }

  stopListening(silent: boolean = true): void {
    if (!this.speechEngine || !this.activeSessionId) {
      if (!silent) {
        promptAction.showToast({ message: 'ASR not started', duration: 2000 });
      }
      return;
    }
    this.speechEngine.finish(this.activeSessionId);
  }

  async startListening(
    context: common.UIAbilityContext,
    callbacks?: AsrCallbacks
  ): Promise<boolean> {
    if (this.activeSessionId) {
      console.info('ASR already active, ignore start request');
      return false;
    }
    try {
      const granted = await this.ensureMicrophonePermission(context);
      if (!granted) {
        const message = 'Microphone permission denied';
        promptAction.showToast({ message, duration: 2000 });
        callbacks?.onError?.(-1, message);
        return false;
      }
      const engine = await this.ensureEngine();
      const sessionId = this.createSessionId();
      this.activeSessionId = sessionId;
      this.activeCallbacks = callbacks;
      const audioInfo: speechRecognizer.AudioInfo = {
        audioType: 'pcm',
        sampleRate: 16000,
        soundChannel: 1,
        sampleBit: 16
      };
      const extraParams: SpeechRecognizerStartExtraParams = {
        recognitionMode: 0,
        vadBegin: 10000,
        vadEnd: 800,
        maxAudioDuration: 20000
      };
      const startParams: speechRecognizer.StartParams = {
        sessionId,
        audioInfo,
        extraParams
      };
      engine.startListening(startParams);
      return true;
    } catch (error) {
      const err = error as BusinessError;
      console.error(`ASR start failed, code: ${err.code}, message: ${err.message}`);
      promptAction.showToast({ message: 'ASR start failed', duration: 2000 });
      callbacks?.onError?.(err.code, err.message);
      this.clearActiveSession();
      return false;
    }
  }

  private createSessionId(): string {
    return `${Date.now()}-${Math.floor(Math.random() * 1000000)}`;
  }

  private setListener(engine: speechRecognizer.SpeechRecognitionEngine): void {
    const self = this;
    const listener: speechRecognizer.RecognitionListener = {
      onStart(sessionId: string, eventMessage: string) {
        console.info(`asr onStart: ${sessionId}, message: ${eventMessage}`);
        if (sessionId !== self.activeSessionId) {
          return;
        }
        self.activeCallbacks?.onStart?.();
      },
      onEvent(sessionId: string, eventCode: number, eventMessage: string) {
        console.info(`asr onEvent: ${sessionId}, code: ${eventCode}, message: ${eventMessage}`);
      },
      onResult(sessionId: string, result: speechRecognizer.SpeechRecognitionResult) {
        console.info(`asr onResult: ${sessionId}, result: ${JSON.stringify(result)}`);
        if (sessionId !== self.activeSessionId) {
          return;
        }
        if (result.isFinal || result.isLast) {
          self.activeCallbacks?.onFinalResult?.(result.result);
        } else {
          self.activeCallbacks?.onPartialResult?.(result.result);
        }
      },
      onComplete(sessionId: string, eventMessage: string) {
        console.info(`asr onComplete: ${sessionId}, message: ${eventMessage}`);
        if (sessionId !== self.activeSessionId) {
          return;
        }
        self.activeCallbacks?.onComplete?.();
        self.clearActiveSession();
      },
      onError(sessionId: string, errorCode: number, errorMessage: string) {
        console.error(`asr onError: ${sessionId}, code: ${errorCode}, message: ${errorMessage}`);
        if (sessionId !== self.activeSessionId) {
          return;
        }
        self.activeCallbacks?.onError?.(errorCode, errorMessage);
        self.clearActiveSession();
      }
    };
    engine.setListener(listener);
  }

  private ensureEngine(): Promise<speechRecognizer.SpeechRecognitionEngine> {
    if (this.speechEngine) {
      return Promise.resolve(this.speechEngine);
    }
    if (this.engineCreatePromise) {
      return this.engineCreatePromise;
    }
    const extraParams: SpeechRecognizerEngineExtraParams = {
      locate: 'CN',
      recognizerMode: 'short'
    };
    const initParamsInfo: speechRecognizer.CreateEngineParams = {
      language: 'zh-CN',
      online: 1,
      extraParams
    };
    this.engineCreatePromise = Promise.resolve()
      .then(() => speechRecognizer.createEngine(initParamsInfo))
      .then((engine) => {
        this.speechEngine = engine;
        this.engineCreatePromise = undefined;
        this.setListener(engine);
        return engine;
      })
      .catch((error: BusinessError) => {
        this.engineCreatePromise = undefined;
        return Promise.reject(error);
      });
    return this.engineCreatePromise;
  }

  private ensureMicrophonePermission(context: common.UIAbilityContext): Promise<boolean> {
    const atManager = abilityAccessCtrl.createAtManager();
    const permission: Permissions = 'ohos.permission.MICROPHONE';
    const tokenId = context.applicationInfo.accessTokenId;
    return atManager.checkAccessToken(tokenId, permission)
      .then((status: abilityAccessCtrl.GrantStatus) => {
        if (status === abilityAccessCtrl.GrantStatus.PERMISSION_GRANTED) {
          return true;
        }
        return atManager.requestPermissionsFromUser(context, [permission])
          .then((result) => {
            const granted = result.authResults.length > 0 &&
              result.authResults.every((item: number) => item === abilityAccessCtrl.GrantStatus.PERMISSION_GRANTED);
            return granted;
          });
      })
      .catch((error: BusinessError) => {
        console.error(`Permission request failed, code: ${error.code}, message: ${error.message}`);
        return false;
      });
  }

  private clearActiveSession(): void {
    this.activeSessionId = '';
    this.activeCallbacks = undefined;
  }
}

export const asrFeature = new AsrFeature();
