export interface FadeVisibilityOptions {
  durationMs?: number;
  delayMs?: number;
  disableHitTest?: boolean;
  fadeInCurve?: Curve;
  fadeOutCurve?: Curve;
  keepSpace?: boolean;
}

@Component
export struct FadeVisibility {
  @Prop @Watch('onVisibleChange') visible: boolean = true;
  @Prop durationMs: number = 180;
  @Prop delayMs: number = 0;
  @Prop disableHitTest: boolean = true;
  @Prop fadeInCurve: Curve = Curve.EaseInOut;
  @Prop fadeOutCurve: Curve = Curve.EaseIn;
  @Prop keepSpace: boolean = false;
  @BuilderParam content: () => void;

  @State private opacityValue: number = 1;
  @State private renderVisible: boolean = true;
  private fadeTimerId: number = -1;
  private initialized: boolean = false;

  aboutToAppear(): void {
    this.syncInitialState();
  }

  aboutToDisappear(): void {
    this.clearTimer();
  }

  private onVisibleChange(): void {
    if (!this.initialized) {
      this.syncInitialState();
      return;
    }
    this.animateVisibility(this.visible);
  }

  private syncInitialState(): void {
    this.opacityValue = this.visible ? 1 : 0;
    this.renderVisible = this.visible;
    this.initialized = true;
  }

  private animateVisibility(nextVisible: boolean): void {
    this.clearTimer();
    const duration = Math.max(0, this.durationMs);
    const delay = Math.max(0, this.delayMs);
    if (nextVisible) {
      this.renderVisible = true;
      this.opacityValue = 0;
      const startDelay = delay + 16;
      this.fadeTimerId = setTimeout(() => {
        if (!this.visible) {
          return;
        }
        animateTo({ duration, curve: this.fadeInCurve }, () => {
          this.opacityValue = 1;
        });
      }, startDelay);
      return;
    }
    animateTo({ duration, delay, curve: this.fadeOutCurve }, () => {
      this.opacityValue = 0;
    });
    this.fadeTimerId = setTimeout(() => {
      if (!this.visible) {
        this.renderVisible = false;
      }
    }, duration + delay);
  }

  private clearTimer(): void {
    if (this.fadeTimerId === -1) {
      return;
    }
    clearTimeout(this.fadeTimerId);
    this.fadeTimerId = -1;
  }

  build() {
    Stack() {
      this.content();
    }
    .opacity(this.opacityValue)
    .visibility(this.renderVisible || this.keepSpace ? Visibility.Visible : Visibility.None)
    .enabled(this.visible || !this.disableHitTest)
  }
}
