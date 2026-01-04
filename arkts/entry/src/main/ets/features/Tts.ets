import { textToSpeech } from '@kit.CoreSpeechKit';
import type { BusinessError } from '@kit.BasicServicesKit';
import { promptAction } from '@kit.ArkUI';

interface TtsEngineExtraParams extends Record<string, Object> {
  style: string;
  locate: string;
  name: string;
}

interface TtsSpeakExtraParams extends Record<string, Object> {
  queueMode: number;
  speed: number;
  volume: number;
  pitch: number;
  languageContext: string;
  audioType: string;
  soundChannel: number;
  playType: number;
}

class TtsFeature {
  private readonly sampleText: string = 'Hello, this is a TTS test.';
  private ttsEngine?: textToSpeech.TextToSpeechEngine;
  private ttsCreatePromise?: Promise<textToSpeech.TextToSpeechEngine>;
  private lastRequestId: string = '';

  startTest(text?: string): void {
    const content = text ?? this.sampleText;
    const preview = content.length > 80 ? `${content.slice(0, 80)}...` : content;
    console.info(`[AS_TTS] speak request len=${content.length} preview="${preview}"`);
    this.ensureTtsEngine()
      .then((engine) => {
        const requestId = this.createRequestId();
        this.lastRequestId = requestId;
        const extraParam: TtsSpeakExtraParams = {
          queueMode: 0,
          speed: 1,
          volume: 2,
          pitch: 1,
          languageContext: 'zh-CN',
          audioType: 'pcm',
          soundChannel: 1,
          playType: 1
        };
        const speakParams: textToSpeech.SpeakParams = {
          requestId,
          extraParams: extraParam
        };
        engine.speak(content, speakParams);
        console.info(`[AS_TTS] speak start requestId=${requestId}`);
        promptAction.showToast({ message: 'TTS started', duration: 2000 });
      })
      .catch((error: BusinessError) => {
        console.error(`TTS failed, code: ${error.code}, message: ${error.message}`);
        promptAction.showToast({ message: 'TTS failed', duration: 2000 });
      });
  }

  stop(): void {
    if (!this.ttsEngine) {
      return;
    }
    try {
      console.info('[AS_TTS] speak stop requested');
      this.ttsEngine.stop();
    } catch (error) {
      console.warn(`TTS stop failed: ${String(error)}`);
    }
  }

  private createRequestId(): string {
    return `${Date.now()}-${Math.floor(Math.random() * 1000000)}`;
  }

  private setListener(engine: textToSpeech.TextToSpeechEngine): void {
    const dataLogged: Set<string> = new Set();
    const speakListener: textToSpeech.SpeakListener = {
      onStart(utteranceId: string, response: textToSpeech.StartResponse) {
        console.info(`[AS_TTS] onStart id=${utteranceId} response=${JSON.stringify(response)}`);
      },
      onComplete(utteranceId: string, response: textToSpeech.CompleteResponse) {
        console.info(`[AS_TTS] onComplete id=${utteranceId} response=${JSON.stringify(response)}`);
      },
      onStop(utteranceId: string, response: textToSpeech.StopResponse) {
        console.info(`[AS_TTS] onStop id=${utteranceId} response=${JSON.stringify(response)}`);
      },
      onData(utteranceId: string, audio: ArrayBuffer, response: textToSpeech.SynthesisResponse) {
        if (!dataLogged.has(utteranceId)) {
          dataLogged.add(utteranceId);
          console.info(
            `[AS_TTS] onData id=${utteranceId} bytes=${audio.byteLength} response=${JSON.stringify(response)}`
          );
        }
      },
      onError(utteranceId: string, errorCode: number, errorMessage: string) {
        console.error(`[AS_TTS] onError id=${utteranceId} code=${errorCode} message=${errorMessage}`);
      }
    };
    engine.setListener(speakListener);
  }

  private ensureTtsEngine(): Promise<textToSpeech.TextToSpeechEngine> {
    if (this.ttsEngine) {
      return Promise.resolve(this.ttsEngine);
    }
    if (this.ttsCreatePromise) {
      return this.ttsCreatePromise;
    }
    const extraParam: TtsEngineExtraParams = { style: 'interaction-broadcast', locate: 'CN', name: 'AgentOperationTts' };
    const initParamsInfo: textToSpeech.CreateEngineParams = {
      language: 'zh-CN',
      person: 0,
      online: 1,
      extraParams: extraParam
    };
    this.ttsCreatePromise = Promise.resolve()
      .then(() => textToSpeech.createEngine(initParamsInfo))
      .then((engine) => {
        this.ttsEngine = engine;
        this.ttsCreatePromise = undefined;
        this.setListener(engine);
        return engine;
      })
      .catch((error: BusinessError) => {
        this.ttsCreatePromise = undefined;
        return Promise.reject(error);
      });
    return this.ttsCreatePromise;
  }
}

export const ttsFeature = new TtsFeature();
