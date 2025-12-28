export interface GlowPalette {
  name: string;
  colors: string[];
}

interface GlowStop {
  color: string;
  location: number;
}

interface GlowLayerOptions {
  lineWidth: number;
  alpha: number;
}

interface GlowRectSpec {
  x: number;
  y: number;
  width: number;
  height: number;
  radius: number;
  lineWidth: number;
}

const DEFAULT_PALETTE_NAME = 'Intelligence';

export const GLOW_PALETTES: GlowPalette[] = [
  {
    name: 'Intelligence',
    colors: ['#BC82F3', '#F5B9EA', '#8D9FFF', '#FF6778', '#FFBA71', '#C686FF']
  },
  {
    name: 'Aurora',
    colors: ['#7DF9FF', '#B5FFD9', '#C3FF68', '#6EF3A5', '#5CB8FF', '#9A7CFF']
  },
  {
    name: 'Sunset',
    colors: ['#FFB000', '#FF7A00', '#FF4D6D', '#FF8FAB', '#FFD166', '#FEE440']
  },
  {
    name: 'Lagoon',
    colors: ['#5BE7C4', '#3DD6FF', '#00A8E8', '#0077B6', '#90E0EF', '#72EFDD']
  },
  {
    name: 'Rose',
    colors: ['#FF6FB1', '#FF9BD1', '#FFC6E8', '#B388FF', '#8C6DFF', '#FF7AA2']
  }
];

@Component
export struct GlowOverlay {
  private readonly settings: RenderingContextSettings = new RenderingContextSettings(true);

  private readonly ctxGlow: CanvasRenderingContext2D = new CanvasRenderingContext2D(this.settings);

  private viewportWidth: number = 0;
  private viewportHeight: number = 0;

  private animationTimerId: number = -1;
  private lastTickMs: number = 0;
  private transitionStartMs: number = 0;
  private rotationRad: number = 0;
  private currentStops: GlowStop[] = [];
  private nextStops: GlowStop[] = [];
  private paletteSignature: string = '';
  private lastVisible: boolean = true;

  @Prop paletteName: string = DEFAULT_PALETTE_NAME;
  @Prop colors: string[] = [];
  @Prop visible: boolean = true;

  aboutToAppear(): void {
    this.lastVisible = this.visible;
    this.resetAnimationState();
    if (this.visible) {
      this.ensureStarted();
    }
  }

  aboutToDisappear(): void {
    this.stop();
  }

  aboutToUpdate(): void {
    if (this.visible === this.lastVisible) {
      return;
    }
    this.lastVisible = this.visible;
    if (this.visible) {
      this.ensureStarted();
      this.drawAll(0);
    } else {
      this.stop();
    }
  }

  private resetAnimationState(): void {
    this.paletteSignature = this.getPaletteColors().join('|');
    this.currentStops = this.generateRandomStops();
    this.nextStops = this.generateRandomStops();
    const now = Date.now();
    this.lastTickMs = now;
    this.transitionStartMs = now;
    this.rotationRad = 0;
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
    if (this.viewportWidth <= 0 || this.viewportHeight <= 0) {
      return;
    }
    if (!this.visible) {
      return;
    }

    this.updatePaletteIfNeeded();

    const now = Date.now();
    const dt = now - this.lastTickMs;
    this.lastTickMs = now;

    const rotationSpeedRadPerMs = 0.0018;
    this.rotationRad = (this.rotationRad + dt * rotationSpeedRadPerMs) % (Math.PI * 2);

    const transitionDurationMs = 650;
    const rawT = (now - this.transitionStartMs) / transitionDurationMs;

    if (rawT >= 1) {
      this.currentStops = this.nextStops;
      this.nextStops = this.generateRandomStops();
      this.transitionStartMs = now;
      this.drawAll(0);
      return;
    }

    const mix = this.easeInOut(Math.min(1, rawT));
    this.drawAll(mix);
  }

  private drawAll(mix: number): void {
    const cornerRadius = this.computeCornerRadius();

    const layers: GlowLayerOptions[] = [
      { lineWidth: 32, alpha: 0.05 },
      { lineWidth: 30, alpha: 0.08 },
      { lineWidth: 28, alpha: 0.12 },
      { lineWidth: 26, alpha: 0.16 },
      { lineWidth: 24, alpha: 0.22 },
      { lineWidth: 20, alpha: 0.3 },
      { lineWidth: 6, alpha: 0.45 }
    ];

    this.drawLayer(this.ctxGlow, layers, cornerRadius, mix);
  }

  private drawLayer(
    ctx: CanvasRenderingContext2D,
    layers: GlowLayerOptions[],
    cornerRadius: number,
    mix: number
  ): void {
    const w = this.viewportWidth;
    const h = this.viewportHeight;
    ctx.clearRect(0, 0, w, h);
    for (let i = 0; i < layers.length; i++) {
      const layer = layers[i];
      if (mix <= 0) {
        this.strokeGlowRect(ctx, layer.lineWidth, layer.alpha, cornerRadius, this.currentStops);
        continue;
      }
      if (mix >= 1) {
        this.strokeGlowRect(ctx, layer.lineWidth, layer.alpha, cornerRadius, this.nextStops);
        continue;
      }
      this.strokeGlowRect(ctx, layer.lineWidth, layer.alpha * (1 - mix), cornerRadius, this.currentStops);
      this.strokeGlowRect(ctx, layer.lineWidth, layer.alpha * mix, cornerRadius, this.nextStops);
    }
  }

  private strokeGlowRect(
    ctx: CanvasRenderingContext2D,
    lineWidth: number,
    alpha: number,
    cornerRadius: number,
    stops: GlowStop[]
  ): void {
    const rect = this.computeRect(lineWidth, cornerRadius);
    ctx.save();
    ctx.globalAlpha = alpha;
    ctx.lineWidth = lineWidth;
    ctx.lineJoin = 'round';
    ctx.lineCap = 'round';
    ctx.strokeStyle = this.createGlowGradient(ctx, this.viewportWidth / 2, this.viewportHeight / 2, this.rotationRad, stops);
    ctx.beginPath();
    this.roundedRectPath(ctx, rect.x, rect.y, rect.width, rect.height, rect.radius);
    ctx.stroke();
    ctx.restore();
  }

  private createGlowGradient(
    ctx: CanvasRenderingContext2D,
    centerX: number,
    centerY: number,
    rotationRad: number,
    stops: GlowStop[]
  ): CanvasGradient {
    const gradient = ctx.createConicGradient(rotationRad, centerX, centerY);
    for (let i = 0; i < stops.length; i++) {
      gradient.addColorStop(stops[i].location, stops[i].color);
    }
    return gradient;
  }

  private roundedRectPath(ctx: CanvasRenderingContext2D, x: number, y: number, width: number, height: number, radius: number): void {
    const r = Math.max(0, Math.min(radius, width / 2, height / 2));
    ctx.moveTo(x + r, y);
    ctx.lineTo(x + width - r, y);
    ctx.arcTo(x + width, y, x + width, y + r, r);
    ctx.lineTo(x + width, y + height - r);
    ctx.arcTo(x + width, y + height, x + width - r, y + height, r);
    ctx.lineTo(x + r, y + height);
    ctx.arcTo(x, y + height, x, y + height - r, r);
    ctx.lineTo(x, y + r);
    ctx.arcTo(x, y, x + r, y, r);
    ctx.closePath();
  }

  private computeRect(lineWidth: number, cornerRadius: number): GlowRectSpec {
    const overscan = Math.min(6, lineWidth * 0.6);
    const x = -overscan;
    const y = -overscan;
    const width = this.viewportWidth + overscan * 2;
    const height = this.viewportHeight + overscan * 2;
    const radius = Math.min(cornerRadius + overscan, width / 2, height / 2);
    return { x, y, width, height, radius, lineWidth };
  }

  private generateRandomStops(): GlowStop[] {
    const palette = this.getPaletteColors();
    const stops: GlowStop[] = [];
    for (let i = 0; i < palette.length; i++) {
      const stop: GlowStop = { color: palette[i], location: Math.random() };
      stops.push(stop);
    }
    stops.sort(this.compareStops);
    if (stops.length > 0) {
      const lastStop: GlowStop = { color: stops[0].color, location: 1 };
      stops.push(lastStop);
    }
    return stops;
  }

  private getPaletteColors(): string[] {
    if (this.colors.length > 0) {
      return this.colors;
    }
    const palette = this.findPaletteByName(this.paletteName);
    if (palette) {
      return palette.colors;
    }
    return GLOW_PALETTES[0].colors;
  }

  private updatePaletteIfNeeded(): void {
    const signature = this.getPaletteColors().join('|');
    if (signature !== this.paletteSignature) {
      this.resetAnimationState();
    }
  }

  private compareStops(a: GlowStop, b: GlowStop): number {
    return a.location - b.location;
  }

  private findPaletteByName(name: string): GlowPalette | null {
    for (let i = 0; i < GLOW_PALETTES.length; i++) {
      const palette = GLOW_PALETTES[i];
      if (palette.name === name) {
        return palette;
      }
    }
    return null;
  }

  private computeCornerRadius(): number {
    const minSide = Math.min(this.viewportWidth, this.viewportHeight);
    return Math.min(80, Math.max(28, minSide * 0.08));
  }

  private easeInOut(t: number): number {
    return t < 0.5 ? 2 * t * t : 1 - Math.pow(-2 * t + 2, 2) / 2;
  }

  build() {
    Stack() {
      Canvas(this.ctxGlow)
        .width('100%')
        .height('100%')
        .enabled(false)
        .onReady(() => {
          if (this.visible) {
            this.ensureStarted();
            this.drawAll(0);
          }
        })
    }
    .width('100%')
    .height('100%')
    .visibility(this.visible ? Visibility.Visible : Visibility.None)
    .onAreaChange((_oldValue, newValue): void => {
      this.viewportWidth = Number(newValue.width);
      this.viewportHeight = Number(newValue.height);
    })
  }
}
