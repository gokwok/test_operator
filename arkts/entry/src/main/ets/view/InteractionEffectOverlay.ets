export interface InteractionEffectOptions {
  durationMs?: number;
  strength?: number;
  color?: string;
}

export interface InteractionPoint {
  x: number;
  y: number;
}

export interface InteractionEffectDefaults {
  tap?: InteractionEffectOptions;
  longPress?: InteractionEffectOptions;
  swipe?: InteractionEffectOptions;
}

export interface InteractionEffectConfig {
  coordScale?: number;
  defaults?: InteractionEffectDefaults;
}

export interface InteractionTapAction {
  kind: 'tap';
  point: InteractionPoint;
  options?: InteractionEffectOptions;
}

export interface InteractionLongPressAction {
  kind: 'long_press';
  point: InteractionPoint;
  options?: InteractionEffectOptions;
}

export interface InteractionSwipeAction {
  kind: 'swipe';
  start: InteractionPoint;
  end: InteractionPoint;
  options?: InteractionEffectOptions;
}

export type InteractionAction = InteractionTapAction | InteractionLongPressAction | InteractionSwipeAction;

type InteractionKind = 'tap' | 'long_press' | 'swipe';

interface InteractionEffectItem {
  id: number;
  kind: InteractionKind;
  x: number;
  y: number;
  endX?: number;
  endY?: number;
  startMs: number;
  durationMs: number;
  strength: number;
  color: string;
}

@Observed
export class InteractionEffectController {
  private nextId: number = 1;
  effects: InteractionEffectItem[] = [];
  private emitListener: (() => void) | null = null;
  coordScale: number = 1000;
  private tapDefaults: InteractionEffectOptions = {};
  private longPressDefaults: InteractionEffectOptions = {};
  private swipeDefaults: InteractionEffectOptions = {};

  static create(): InteractionEffectController {
    return new InteractionEffectController();
  }

  configure(config?: InteractionEffectConfig): void {
    if (!config) {
      return;
    }
    if (config.coordScale !== undefined) {
      this.coordScale = Math.max(1, config.coordScale);
    }
    if (config.defaults?.tap) {
      this.tapDefaults = this.copyOptions(config.defaults.tap);
    }
    if (config.defaults?.longPress) {
      this.longPressDefaults = this.copyOptions(config.defaults.longPress);
    }
    if (config.defaults?.swipe) {
      this.swipeDefaults = this.copyOptions(config.defaults.swipe);
    }
  }

  bindEmitter(listener?: () => void): void {
    this.emitListener = listener ?? null;
  }

  play(action: InteractionAction): void {
    if (action.kind === 'tap') {
      this.tap(action.point, action.options);
      return;
    }
    if (action.kind === 'long_press') {
      this.longPress(action.point, action.options);
      return;
    }
    this.swipe(action.start, action.end, action.options);
  }

  tap(point: InteractionPoint, options?: InteractionEffectOptions): void {
    const resolved = this.resolveOptions('tap', options);
    this.pushEffect('tap', point, null, resolved, 120);
  }

  longPress(point: InteractionPoint, options?: InteractionEffectOptions): void {
    const resolved = this.resolveOptions('long_press', options);
    this.pushEffect('long_press', point, null, resolved, 300);
  }

  swipe(start: InteractionPoint, end: InteractionPoint, options?: InteractionEffectOptions): void {
    const resolved = this.resolveOptions('swipe', options);
    this.pushEffect('swipe', start, end, resolved, 200);
  }


  clear(): void {
    this.effects = [];
  }

  prune(now: number): void {
    if (this.effects.length === 0) {
      return;
    }
    const next: InteractionEffectItem[] = [];
    for (let i = 0; i < this.effects.length; i++) {
      const effect = this.effects[i];
      if (now - effect.startMs <= effect.durationMs) {
        next.push(effect);
      }
    }
    if (next.length !== this.effects.length) {
      this.effects = next;
    }
  }

  private clampRelative(value: number): number {
    if (value < 0) {
      return 0;
    }
    if (value > this.coordScale) {
      return this.coordScale;
    }
    return value;
  }

  private pushEffect(
    kind: InteractionKind,
    point: InteractionPoint,
    end: InteractionPoint | null,
    options: InteractionEffectOptions,
    minDuration: number
  ): void {
    const durationMs = options.durationMs ?? 520;
    const strength = options.strength ?? 1;
    const color = options.color ?? '#FFFFFF';
    const clampedX = this.clampRelative(point.x);
    const clampedY = this.clampRelative(point.y);
    const effect: InteractionEffectItem = {
      id: this.nextId++,
      kind,
      x: clampedX,
      y: clampedY,
      startMs: Date.now(),
      durationMs: Math.max(minDuration, durationMs),
      strength: Math.max(0.2, strength),
      color
    };
    if (end) {
      effect.endX = this.clampRelative(end.x);
      effect.endY = this.clampRelative(end.y);
    }
    this.effects = [...this.effects, effect];
    if (this.emitListener) {
      this.emitListener();
    }
  }

  private resolveOptions(kind: InteractionKind, options?: InteractionEffectOptions): InteractionEffectOptions {
    if (kind === 'tap') {
      return this.mergeOptions(this.tapDefaults, options);
    }
    if (kind === 'long_press') {
      return this.mergeOptions(this.longPressDefaults, options);
    }
    return this.mergeOptions(this.swipeDefaults, options);
  }

  private mergeOptions(base: InteractionEffectOptions, override?: InteractionEffectOptions): InteractionEffectOptions {
    const merged: InteractionEffectOptions = this.copyOptions(base);
    if (override) {
      if (override.durationMs !== undefined) {
        merged.durationMs = override.durationMs;
      }
      if (override.strength !== undefined) {
        merged.strength = override.strength;
      }
      if (override.color !== undefined) {
        merged.color = override.color;
      }
    }
    return merged;
  }

  private copyOptions(source: InteractionEffectOptions): InteractionEffectOptions {
    const result: InteractionEffectOptions = {};
    if (source.durationMs !== undefined) {
      result.durationMs = source.durationMs;
    }
    if (source.strength !== undefined) {
      result.strength = source.strength;
    }
    if (source.color !== undefined) {
      result.color = source.color;
    }
    return result;
  }
}

@Component
export struct InteractionEffectOverlay {
  private readonly settings: RenderingContextSettings = new RenderingContextSettings(true);
  private readonly ctx: CanvasRenderingContext2D = new CanvasRenderingContext2D(this.settings);

  private viewportWidth: number = 0;
  private viewportHeight: number = 0;
  private animationTimerId: number = -1;
  private lastVisible: boolean = true;

  @ObjectLink controller: InteractionEffectController;
  @Prop visible: boolean = true;
  @Prop baseColor: string = '#FFFFFF';

  aboutToAppear(): void {
    this.lastVisible = this.visible;
    this.bindEmitter();
    if (this.visible) {
      this.ensureStarted();
    }
  }

  aboutToDisappear(): void {
    this.controller.bindEmitter();
    this.stop();
  }

  aboutToUpdate(): void {
    this.bindEmitter();
    if (this.visible === this.lastVisible) {
      return;
    }
    this.lastVisible = this.visible;
    if (this.visible) {
      this.ensureStarted();
    } else {
      this.stop();
      this.clearCanvas();
    }
  }

  private ensureStarted(): void {
    if (this.animationTimerId !== -1) {
      return;
    }
    this.animationTimerId = setInterval(() => this.tick(), 16);
  }

  private stop(): void {
    if (this.animationTimerId === -1) {
      return;
    }
    clearInterval(this.animationTimerId);
    this.animationTimerId = -1;
  }

  private tick(): void {
    if (!this.visible || this.viewportWidth <= 0 || this.viewportHeight <= 0) {
      return;
    }
    const now = Date.now();
    this.controller.prune(now);
    if (this.controller.effects.length === 0) {
      this.clearCanvas();
      this.stop();
      return;
    }
    this.draw(now);
  }

  private clearCanvas(): void {
    if (this.viewportWidth <= 0 || this.viewportHeight <= 0) {
      return;
    }
    this.ctx.clearRect(0, 0, this.viewportWidth, this.viewportHeight);
  }

  private draw(now: number): void {
    const w = this.viewportWidth;
    const h = this.viewportHeight;
    this.ctx.clearRect(0, 0, w, h);
    for (let i = 0; i < this.controller.effects.length; i++) {
      this.drawEffect(this.controller.effects[i], now);
    }
  }

  private drawEffect(effect: InteractionEffectItem, now: number): void {
    if (effect.kind === 'tap') {
      this.drawTap(effect, now);
      return;
    }
    if (effect.kind === 'long_press') {
      this.drawLongPress(effect, now);
      return;
    }
    this.drawSwipe(effect, now);
  }

  private drawTap(effect: InteractionEffectItem, now: number): void {
    const elapsed = now - effect.startMs;
    if (elapsed < 0) {
      return;
    }
    const progress = Math.min(1, elapsed / effect.durationMs);
    const eased = this.easeOutCubic(progress);
    const radius = this.lerp(10, 38, eased) * effect.strength;
    const ringWidth = this.lerp(4.4, 1.8, eased) * effect.strength;
    const alpha = Math.pow(1 - progress, 1.05);
    const x = this.toX(effect.x);
    const y = this.toY(effect.y);
    const color = effect.color || this.baseColor;

    this.ctx.save();
    this.ctx.globalAlpha = alpha * 0.95;
    this.ctx.strokeStyle = color;
    this.ctx.lineWidth = ringWidth;
    this.ctx.shadowBlur = 26 * effect.strength;
    this.ctx.shadowColor = color;
    this.ctx.beginPath();
    this.ctx.arc(x, y, radius, 0, Math.PI * 2);
    this.ctx.stroke();
    this.ctx.restore();

    this.ctx.save();
    this.ctx.globalAlpha = alpha * 0.5;
    this.ctx.strokeStyle = color;
    this.ctx.lineWidth = ringWidth * 2.2;
    this.ctx.shadowBlur = 36 * effect.strength;
    this.ctx.shadowColor = color;
    this.ctx.beginPath();
    this.ctx.arc(x, y, radius * 0.7, 0, Math.PI * 2);
    this.ctx.stroke();
    this.ctx.restore();

    const dotRadius = this.lerp(5.6, 0, progress) * effect.strength;
    if (dotRadius > 0.4) {
      this.ctx.save();
      this.ctx.globalAlpha = alpha;
      this.ctx.fillStyle = color;
      this.ctx.shadowBlur = 18 * effect.strength;
      this.ctx.shadowColor = color;
      this.ctx.beginPath();
      this.ctx.arc(x, y, dotRadius, 0, Math.PI * 2);
      this.ctx.fill();
      this.ctx.restore();
    }
  }

  private drawLongPress(effect: InteractionEffectItem, now: number): void {
    const elapsed = now - effect.startMs;
    if (elapsed < 0) {
      return;
    }
    const progress = Math.min(1, elapsed / effect.durationMs);
    const attack = Math.min(1, progress / 0.25);
    const release = progress < 0.85 ? 0 : (progress - 0.85) / 0.15;
    const holdPulse = 1 + Math.sin(progress * Math.PI * 4) * 0.03;
    const radius = this.lerp(12, 34, this.easeOutCubic(attack)) * holdPulse * effect.strength;
    const ringWidth = this.lerp(5.2, 2.4, attack) * effect.strength;
    const alpha = (1 - release) * 0.9;
    const x = this.toX(effect.x);
    const y = this.toY(effect.y);
    const color = effect.color || this.baseColor;

    this.ctx.save();
    this.ctx.globalAlpha = alpha * 0.9;
    this.ctx.strokeStyle = color;
    this.ctx.lineWidth = ringWidth;
    this.ctx.shadowBlur = 28 * effect.strength;
    this.ctx.shadowColor = color;
    this.ctx.beginPath();
    this.ctx.arc(x, y, radius, 0, Math.PI * 2);
    this.ctx.stroke();
    this.ctx.restore();

    this.ctx.save();
    this.ctx.globalAlpha = alpha * 0.5;
    this.ctx.fillStyle = color;
    this.ctx.shadowBlur = 20 * effect.strength;
    this.ctx.shadowColor = color;
    this.ctx.beginPath();
    this.ctx.arc(x, y, 6 * effect.strength, 0, Math.PI * 2);
    this.ctx.fill();
    this.ctx.restore();
  }

  private drawSwipe(effect: InteractionEffectItem, now: number): void {
    if (effect.endX === undefined || effect.endY === undefined) {
      return;
    }
    const elapsed = now - effect.startMs;
    if (elapsed < 0) {
      return;
    }
    const progress = Math.min(1, elapsed / effect.durationMs);
    const eased = this.easeOutCubic(progress);
    const alpha = Math.pow(1 - progress, 1.1);
    const color = effect.color || this.baseColor;
    const x1 = this.toX(effect.x);
    const y1 = this.toY(effect.y);
    const x2 = this.toX(effect.endX);
    const y2 = this.toY(effect.endY);

    const lineWidth = this.lerp(5, 2.6, eased) * effect.strength;
    this.ctx.save();
    this.ctx.globalAlpha = alpha * 0.6;
    this.ctx.strokeStyle = color;
    this.ctx.lineWidth = lineWidth;
    this.ctx.lineCap = 'round';
    this.ctx.shadowBlur = 18 * effect.strength;
    this.ctx.shadowColor = color;
    this.ctx.beginPath();
    this.ctx.moveTo(x1, y1);
    this.ctx.lineTo(x2, y2);
    this.ctx.stroke();
    this.ctx.restore();

    const startRadius = this.lerp(6, 4, eased) * effect.strength;
    this.ctx.save();
    this.ctx.globalAlpha = alpha * 0.75;
    this.ctx.fillStyle = color;
    this.ctx.shadowBlur = 14 * effect.strength;
    this.ctx.shadowColor = color;
    this.ctx.beginPath();
    this.ctx.arc(x1, y1, startRadius, 0, Math.PI * 2);
    this.ctx.fill();
    this.ctx.restore();

    const endRadius = this.lerp(11, 7, eased) * effect.strength;
    const endRingWidth = this.lerp(3.6, 2.2, eased) * effect.strength;
    this.ctx.save();
    this.ctx.globalAlpha = alpha * 0.85;
    this.ctx.strokeStyle = color;
    this.ctx.lineWidth = endRingWidth;
    this.ctx.shadowBlur = 20 * effect.strength;
    this.ctx.shadowColor = color;
    this.ctx.beginPath();
    this.ctx.arc(x2, y2, endRadius, 0, Math.PI * 2);
    this.ctx.stroke();
    this.ctx.restore();

    const headX = this.lerp(x1, x2, eased);
    const headY = this.lerp(y1, y2, eased);
    const headRadius = this.lerp(7, 3, eased) * effect.strength;
    this.ctx.save();
    this.ctx.globalAlpha = alpha * 0.95;
    this.ctx.fillStyle = color;
    this.ctx.shadowBlur = 24 * effect.strength;
    this.ctx.shadowColor = color;
    this.ctx.beginPath();
    this.ctx.arc(headX, headY, headRadius, 0, Math.PI * 2);
    this.ctx.fill();
    this.ctx.restore();
  }

  private lerp(a: number, b: number, t: number): number {
    return a + (b - a) * t;
  }

  private toX(relativeX: number): number {
    return this.viewportWidth * (relativeX / this.controller.coordScale);
  }

  private toY(relativeY: number): number {
    return this.viewportHeight * (relativeY / this.controller.coordScale);
  }

  private easeOutCubic(t: number): number {
    return 1 - Math.pow(1 - t, 3);
  }

  private bindEmitter(): void {
    this.controller.bindEmitter(() => {
      if (!this.visible) {
        return;
      }
      this.ensureStarted();
    });
  }

  build() {
    Stack() {
      Canvas(this.ctx)
        .width('100%')
        .height('100%')
        .enabled(false)
        .onReady(() => {
          if (this.visible) {
            this.ensureStarted();
          }
        })
    }
    .width('100%')
    .height('100%')
    .enabled(false)
    .visibility(this.visible ? Visibility.Visible : Visibility.None)
    .onAreaChange((_oldValue, newValue): void => {
      this.viewportWidth = Number(newValue.width);
      this.viewportHeight = Number(newValue.height);
    })
  }
}
