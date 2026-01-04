import { abilityAccessCtrl, common, Permissions } from '@kit.AbilityKit';
import { speechRecognizer } from '@kit.CoreSpeechKit';
import type { BusinessError } from '@kit.BasicServicesKit';
import { promptAction } from '@kit.ArkUI';

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

  startTest(context: common.UIAbilityContext): void {
    this.ensureMicrophonePermission(context)
      .then((granted: boolean) => {
        if (!granted) {
          promptAction.showToast({ message: 'Microphone permission denied', duration: 2000 });
          return;
        }
        return this.ensureEngine().then((engine) => {
          const sessionId = this.createSessionId();
          this.activeSessionId = sessionId;
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
          promptAction.showToast({ message: 'ASR started', duration: 2000 });
        });
      })
      .catch((error: BusinessError) => {
        console.error(`ASR start failed, code: ${error.code}, message: ${error.message}`);
        promptAction.showToast({ message: 'ASR start failed', duration: 2000 });
      });
  }

  finish(): void {
    if (!this.speechEngine || !this.activeSessionId) {
      promptAction.showToast({ message: 'ASR not started', duration: 2000 });
      return;
    }
    this.speechEngine.finish(this.activeSessionId);
  }

  private createSessionId(): string {
    return `${Date.now()}-${Math.floor(Math.random() * 1000000)}`;
  }

  private setListener(engine: speechRecognizer.SpeechRecognitionEngine): void {
    const listener: speechRecognizer.RecognitionListener = {
      onStart(sessionId: string, eventMessage: string) {
        console.info(`asr onStart: ${sessionId}, message: ${eventMessage}`);
      },
      onEvent(sessionId: string, eventCode: number, eventMessage: string) {
        console.info(`asr onEvent: ${sessionId}, code: ${eventCode}, message: ${eventMessage}`);
      },
      onResult(sessionId: string, result: speechRecognizer.SpeechRecognitionResult) {
        console.info(`asr onResult: ${sessionId}, result: ${JSON.stringify(result)}`);
        if (result.isFinal || result.isLast) {
          promptAction.showToast({ message: `ASR: ${result.result}`, duration: 2000 });
        }
      },
      onComplete(sessionId: string, eventMessage: string) {
        console.info(`asr onComplete: ${sessionId}, message: ${eventMessage}`);
      },
      onError(sessionId: string, errorCode: number, errorMessage: string) {
        console.error(`asr onError: ${sessionId}, code: ${errorCode}, message: ${errorMessage}`);
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
}

export const asrFeature = new AsrFeature();
