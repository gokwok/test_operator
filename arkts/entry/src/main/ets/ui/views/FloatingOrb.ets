import display from '@ohos.display';

export type FloatingOrbMode = 'compact' | 'expanded';

interface OrbSize {
  width: number;
  height: number;
}

interface OrbPosition {
  x: number;
  y: number;
}

@Observed
export class FloatingOrbController {
  visible: boolean = true;
  mode: FloatingOrbMode = 'compact';
  text: string = '';

  private clickHandler: () => void = () => {};

  static create(text?: string, mode?: FloatingOrbMode, onClick?: () => void): FloatingOrbController {
    const controller = new FloatingOrbController();
    if (text) {
      controller.text = text;
    }
    if (mode) {
      controller.mode = mode;
    }
    if (onClick) {
      controller.clickHandler = onClick;
    }
    return controller;
  }

  show(): void {
    this.visible = true;
  }

  hide(): void {
    this.visible = false;
  }

  toggle(): void {
    this.visible = !this.visible;
  }

  setMode(mode: FloatingOrbMode): void {
    this.mode = mode;
  }

  setText(text: string): void {
    this.text = text;
  }

  setClickHandler(handler: () => void): void {
    this.clickHandler = handler;
  }

  triggerClick(): void {
    this.clickHandler();
  }

}

@Component
export struct FloatingOrb {
  @ObjectLink controller: FloatingOrbController;
  @Prop @Watch('onVisibleChange') visible: boolean = true;
  @Prop edgePadding: number = 12;
  @Prop compactSize: number = 52;
  @Prop expandedWidth: number = 176;
  @Prop expandedHeight: number = 52;
  @Prop animationDurationMs: number = 220;

  @State private animX: number = 0;
  @State private animY: number = 0;
  @State private opacityValue: number = 1;
  @State private scaleValue: number = 1;
  @State private renderVisible: boolean = true;

  private readonly glassSurfaceColor: string = '#66FFFFFF';
  private readonly glassSurfaceStrongColor: string = '#80FFFFFF';
  private readonly glassBorderColor: string = '#80FFFFFF';
  private readonly glassShadowSoftColor: string = '#14000000';
  private readonly glassBlurRadius: number = 18;
  private readonly textPrimaryColor: string = '#2B2D35';

  private viewportWidth: number = 0;
  private viewportHeight: number = 0;
  private lastMode: FloatingOrbMode = 'compact';
  private restX: number = 0;
  private restY: number = 0;
  private hasRestPosition: boolean = false;
  private dragStartX: number = 0;
  private dragStartY: number = 0;
  private dragMoved: boolean = false;
  private hideTimerId: number = -1;
  private visibilityToken: number = 0;
  private suppressClick: boolean = false;
  private suppressClickTimerId: number = -1;
  private initialized: boolean = false;
  private visibilityAnimating: boolean = false;
  private snapTimerId: number = -1;

  aboutToAppear(): void {
    this.updateViewport();
    this.lastMode = this.controller.mode;
    this.renderVisible = this.visible;
    this.opacityValue = this.visible ? 1 : 0;
    this.scaleValue = this.visible ? 1 : 0.86;
    if (this.visible) {
      this.ensureDefaultPosition();
    }
    this.initialized = true;
  }

  aboutToDisappear(): void {
    this.clearTimer();
    this.clearSuppressClickTimer();
    this.clearSnapTimer();
  }

  aboutToUpdate(): void {
    this.updateViewport();
    if (this.controller.mode !== this.lastMode) {
      this.lastMode = this.controller.mode;
      this.adjustForMode();
    }
    if (this.controller.mode === 'compact' && !this.dragMoved) {
      if (this.visible && this.renderVisible && !this.visibilityAnimating) {
        this.snapToEdge(true);
      }
    }
  }

  private onVisibleChange(): void {
    if (!this.initialized) {
      return;
    }
    this.handleVisibility(this.visible);
  }

  private handleVisibility(nextVisible: boolean): void {
    this.animateVisibility(nextVisible);
  }

  private updateViewport(): void {
    try {
      const viewport = this.getViewportFromDisplay();
      this.viewportWidth = viewport.width;
      this.viewportHeight = viewport.height;
    } catch (e) {
      this.viewportWidth = 0;
      this.viewportHeight = 0;
    }
  }

  private getViewportFromDisplay(): OrbSize {
    const defaultDisplay = display.getDefaultDisplaySync();
    const densityPixels = Number(defaultDisplay.densityPixels ?? 1);
    const widthPx = Number(defaultDisplay.width ?? 0);
    const heightPx = Number(defaultDisplay.height ?? 0);
    const density = densityPixels > 0 ? densityPixels : 1;
    const width = widthPx > 0 ? widthPx / density : 0;
    const height = heightPx > 0 ? heightPx / density : 0;
    return { width: width > 0 ? width : 360, height: height > 0 ? height : 780 };
  }

  private ensureDefaultPosition(): void {
    if (this.hasRestPosition || this.viewportWidth <= 0 || this.viewportHeight <= 0) {
      return;
    }
    const size = this.getBallSize(this.controller.mode);
    const defaultX = this.viewportWidth - size.width - this.edgePadding;
    const defaultY = this.viewportHeight * 0.58;
    const clamped = this.clampPosition(defaultX, defaultY, size);
    this.restX = clamped.x;
    this.restY = clamped.y;
    this.hasRestPosition = true;
    this.animX = clamped.x;
    this.animY = clamped.y;
  }

  private adjustForMode(): void {
    if (this.viewportWidth <= 0 || this.viewportHeight <= 0) {
      return;
    }
    const size = this.getBallSize(this.controller.mode);
    const clamped = this.clampPosition(this.animX, this.animY, size);
    this.restX = clamped.x;
    this.restY = clamped.y;
    this.animX = clamped.x;
    this.animY = clamped.y;
    if (this.controller.mode === 'compact') {
      this.scheduleSnapToEdge();
    }
  }

  private getEffectiveX(): number {
    if (this.viewportWidth <= 0) {
      return this.animX;
    }
    if (this.visibilityAnimating) {
      return this.animX;
    }
    const size = this.getBallSize(this.controller.mode);
    const maxX = Math.max(this.edgePadding, this.viewportWidth - size.width - this.edgePadding);
    return Math.min(Math.max(this.animX, this.edgePadding), maxX);
  }

  private getEffectiveY(): number {
    if (this.viewportHeight <= 0) {
      return this.animY;
    }
    if (this.visibilityAnimating) {
      return this.animY;
    }
    const size = this.getBallSize(this.controller.mode);
    const offscreenMargin = size.height * 0.35;
    const offscreenTop = -offscreenMargin;
    const offscreenBottom = this.viewportHeight + offscreenMargin;
    if (this.animY < offscreenTop || this.animY > offscreenBottom) {
      return this.animY;
    }
    const maxY = Math.max(this.edgePadding, this.viewportHeight - size.height - this.edgePadding);
    return Math.min(Math.max(this.animY, this.edgePadding), maxY);
  }

  private animateVisibility(nextVisible: boolean): void {
    this.clearTimer();
    const token = ++this.visibilityToken;
    const duration = Math.max(120, this.animationDurationMs);
    this.visibilityAnimating = true;
    if (this.viewportWidth <= 0 || this.viewportHeight <= 0) {
      this.updateViewport();
    }
    const size = this.getBallSize(this.controller.mode);
    const origin = this.getOriginPosition(size);
    if (nextVisible) {
      this.ensureDefaultPosition();
      this.renderVisible = true;
      this.animX = origin.x;
      this.animY = origin.y;
      this.opacityValue = 0;
      this.scaleValue = 0.82;
      const startDelay = 16;
      this.hideTimerId = setTimeout(() => {
        if (this.visibilityToken !== token || !this.visible) {
          this.visibilityAnimating = false;
          return;
        }
        animateTo({ duration, curve: Curve.EaseOut }, () => {
          this.animX = this.restX;
          this.animY = this.restY;
          this.opacityValue = 1;
          this.scaleValue = 1;
        });
        this.clearTimer();
        this.hideTimerId = setTimeout(() => {
          if (this.visibilityToken === token && this.visible) {
            this.visibilityAnimating = false;
          }
        }, duration);
      }, startDelay);
      return;
    }
    this.renderVisible = true;
    animateTo({ duration, curve: Curve.EaseIn }, () => {
      this.animX = origin.x;
      this.animY = origin.y;
      this.opacityValue = 0;
      this.scaleValue = 0.84;
    });
    this.hideTimerId = setTimeout(() => {
      if (this.visibilityToken !== token) {
        return;
      }
      if (!this.visible) {
        this.renderVisible = false;
      }
      this.visibilityAnimating = false;
    }, duration);
  }

  private clearTimer(): void {
    if (this.hideTimerId === -1) {
      return;
    }
    clearTimeout(this.hideTimerId);
    this.hideTimerId = -1;
  }

  private clearSuppressClickTimer(): void {
    if (this.suppressClickTimerId === -1) {
      return;
    }
    clearTimeout(this.suppressClickTimerId);
    this.suppressClickTimerId = -1;
  }

  private scheduleSnapToEdge(): void {
    this.clearSnapTimer();
    this.snapTimerId = setTimeout(() => {
      this.snapTimerId = -1;
      if (this.visible && this.renderVisible && !this.visibilityAnimating) {
        this.snapToEdge(true);
      }
    }, 16);
  }

  private clearSnapTimer(): void {
    if (this.snapTimerId === -1) {
      return;
    }
    clearTimeout(this.snapTimerId);
    this.snapTimerId = -1;
  }

  private getBallSize(mode: FloatingOrbMode): OrbSize {
    if (mode === 'expanded') {
      return { width: this.expandedWidth, height: this.expandedHeight };
    }
    return { width: this.compactSize, height: this.compactSize };
  }

  private clampPosition(x: number, y: number, size: OrbSize): OrbPosition {
    const maxX = Math.max(this.edgePadding, this.viewportWidth - size.width - this.edgePadding);
    const maxY = Math.max(this.edgePadding, this.viewportHeight - size.height - this.edgePadding);
    const clampedX = Math.min(Math.max(x, this.edgePadding), maxX);
    const clampedY = Math.min(Math.max(y, this.edgePadding), maxY);
    return { x: clampedX, y: clampedY };
  }

  private getOriginPosition(size: OrbSize): OrbPosition {
    const x = (this.viewportWidth - size.width) / 2;
    const y = this.viewportHeight + size.height * 0.6;
    return { x, y };
  }

  private updatePosition(nextX: number, nextY: number): void {
    const size = this.getBallSize(this.controller.mode);
    const clamped = this.clampPosition(nextX, nextY, size);
    this.animX = clamped.x;
    this.animY = clamped.y;
    this.restX = clamped.x;
    this.restY = clamped.y;
  }

  private onPanStart(): void {
    this.clearTimer();
    this.clearSuppressClickTimer();
    this.suppressClick = false;
    this.dragStartX = this.restX;
    this.dragStartY = this.restY;
    this.dragMoved = false;
  }

  private onPanUpdate(offsetX: number, offsetY: number): void {
    const nextX = this.dragStartX + offsetX;
    const nextY = this.dragStartY + offsetY;
    this.updatePosition(nextX, nextY);
    if (Math.abs(offsetX) + Math.abs(offsetY) > 2) {
      this.dragMoved = true;
      this.suppressClick = true;
    }
  }

  private onPanEnd(): void {
    if (!this.dragMoved) {
      this.suppressClick = false;
      return;
    }
    this.dragMoved = false;
    this.snapToEdge(true);
    this.clearSuppressClickTimer();
    this.suppressClickTimerId = setTimeout(() => {
      this.suppressClick = false;
      this.suppressClickTimerId = -1;
    }, 180);
  }

  private snapToEdge(animated: boolean): void {
    if (this.viewportWidth <= 0 || this.viewportHeight <= 0) {
      this.updateViewport();
    }
    if (this.viewportWidth <= 0 || this.viewportHeight <= 0) {
      return;
    }
    const size = this.getBallSize(this.controller.mode);
    const leftX = this.edgePadding;
    const rightX = Math.max(this.edgePadding, this.viewportWidth - size.width - this.edgePadding);
    const distLeft = Math.abs(this.restX - leftX);
    const distRight = Math.abs(this.restX - rightX);
    const targetX = distLeft <= distRight ? leftX : rightX;
    if (Math.abs(this.restX - targetX) < 0.5) {
      return;
    }
    const clamped = this.clampPosition(targetX, this.restY, size);
    this.restX = clamped.x;
    this.restY = clamped.y;
    if (!animated) {
      this.animX = clamped.x;
      this.animY = clamped.y;
      return;
    }
    animateTo({ duration: 180, curve: Curve.EaseOut }, () => {
      this.animX = clamped.x;
      this.animY = clamped.y;
    });
  }

  @Builder
  private playIcon() {
    SymbolGlyph($r('sys.symbol.play_fill'))
      .fontSize(18)
      .fontColor(['#FF7A45'])
  }

  @Builder
  private compactBall() {
    Row() {
      this.playIcon();
    }
    .width(this.compactSize)
    .height(this.compactSize)
    .borderRadius(this.compactSize / 2)
    .backdropBlur(this.glassBlurRadius)
    .backgroundColor(this.glassSurfaceStrongColor)
    .borderWidth(1)
    .borderColor(this.glassBorderColor)
    .justifyContent(FlexAlign.Center)
    .alignItems(VerticalAlign.Center)
    .shadow({ radius: 16, color: this.glassShadowSoftColor, offsetX: 0, offsetY: 8 })
    .clip(true)
  }

  @Builder
  private expandedBall() {
    Stack() {
      Row({ space: 8 }) {
        Row() {
          this.playIcon();
        }
        .width(this.expandedHeight)
        .height(this.expandedHeight)
        .justifyContent(FlexAlign.Center)
        .alignItems(VerticalAlign.Center)

      Text(this.controller.text)
        .fontSize(13)
        .fontColor(this.textPrimaryColor)
        .fontWeight(FontWeight.Medium)
        .maxLines(1)
        .layoutWeight(1)
        .textOverflow({ overflow: TextOverflow.Clip })
      }
      .width('100%')
      .height('100%')
      .padding({ right: 14 })
      .alignItems(VerticalAlign.Center)
    }
    .width(this.expandedWidth)
    .height(this.expandedHeight)
    .borderRadius(this.expandedHeight / 2 + 1)
    .backdropBlur(this.glassBlurRadius)
    .backgroundColor(this.glassSurfaceColor)
    .borderWidth(1)
    .borderColor(this.glassBorderColor)
    .shadow({ radius: 16, color: this.glassShadowSoftColor, offsetX: 0, offsetY: 8 })
    .clip(true)
  }

  @Builder
  private orbContent() {
    Stack() {
      if (this.controller.mode === 'expanded') {
        this.expandedBall();
      } else {
        this.compactBall();
      }
    }
    .width(this.controller.mode === 'expanded' ? this.expandedWidth : this.compactSize)
    .height(this.controller.mode === 'expanded' ? this.expandedHeight : this.compactSize)
    .position({ x: this.getEffectiveX(), y: this.getEffectiveY() })
    .opacity(this.opacityValue)
    .scale({ x: this.scaleValue, y: this.scaleValue })
    .gesture(
      PanGesture()
        .onActionStart(() => {
          this.onPanStart();
        })
        .onActionUpdate((event) => {
          const offsetX = Number(event.offsetX ?? 0);
          const offsetY = Number(event.offsetY ?? 0);
          this.onPanUpdate(offsetX, offsetY);
        })
        .onActionEnd(() => {
          this.onPanEnd();
        })
    )
    .onClick(() => {
      if (this.suppressClick) {
        return;
      }
      this.controller.triggerClick();
    })
    .visibility(this.visible || this.renderVisible ? Visibility.Visible : Visibility.None)
  }

  build() {
    this.orbContent();
  }
}
