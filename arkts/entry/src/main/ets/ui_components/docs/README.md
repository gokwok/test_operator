# UI Components Usage

This folder documents the custom ArkTS UI components under `entry/src/main/ets/ui_components`.

## GlowOverlay

Purpose: animated edge glow for a full-screen stack.

Exports:
- `GlowOverlay`
- `GLOW_PALETTES`
- `GlowPalette` (name + colors)

Props:
- `paletteName`: palette name from `GLOW_PALETTES`
- `colors`: optional custom colors (string array)
- `visible`: show/hide and start/stop rendering

Example:
```ts
import { GlowOverlay, GLOW_PALETTES } from '../ui_components/GlowOverlay';

@State private paletteIndex: number = 0;

GlowOverlay({ paletteName: GLOW_PALETTES[this.paletteIndex].name, visible: true })
  .width('100%')
  .height('100%')
```

## FadeVisibility

Purpose: fade in/out wrapper for arbitrary content.

Props:
- `visible`: show/hide
- `durationMs`: fade duration
- `delayMs`: start delay
- `disableHitTest`: disable input when hidden
- `fadeInCurve`, `fadeOutCurve`: animation curves
- `keepSpace`: keep layout space when hidden

Example:
```ts
import { FadeVisibility } from '../ui_components/FadeVisibility';

FadeVisibility({ visible: this.showPanel, durationMs: 200 }) {
  Column() {
    Text('Panel')
  }
}
```

## StatusFloatWindow

Purpose: status float window with actions and a stopped state.

Controller:
- `StatusFloatWindowController.create(statusText?, supplement?, takeover?, stop?, destroy?)`
- `setStatus(text)`
- `setStopped(stopped)`
- `setSupplementHandler(handler)`
- `setTakeoverHandler(handler)`
- `setStopHandler(handler)`
- `setDestroyHandler(handler)`

Component props:
- `controller`
- `visible`

Example:
```ts
import { StatusFloatWindow, StatusFloatWindowController } from '../ui_components/StatusFloatWindow';

@State private statusController = StatusFloatWindowController.create(
  'hello world',
  () => {},
  () => {},
  () => {},
  () => {}
);

StatusFloatWindow({ controller: this.statusController, visible: true })
  .width('100%')
```

## InteractionEffectOverlay

Purpose: render interaction effects (tap, long press, swipe) based on relative coordinates.

Coordinate system:
- Default scale is 1000 (0..1000)
- Top-left is (0, 0)

Controller:
- `InteractionEffectController.create()`
- `configure({ coordScale, defaults })`
- `tap(point, options?)`
- `longPress(point, options?)`
- `swipe(start, end, options?)`
- `play(action)`
- `clear()`

Component props:
- `controller`
- `visible`
- `baseColor`

Example:
```ts
import {
  InteractionEffectController,
  InteractionEffectOverlay
} from '../ui_components/InteractionEffectOverlay';

@State private interactionController = InteractionEffectController.create();

InteractionEffectOverlay({ controller: this.interactionController })
  .width('100%')
  .height('100%')

this.interactionController.tap({ x: 300, y: 500 });
```

## FloatingOrb

Purpose: draggable floating orb with compact and expanded modes.

Controller:
- `FloatingOrbController.create(text?, mode?, onClick?)`
- `show()`, `hide()`, `toggle()`
- `setMode(mode)`
- `setText(text)`
- `setClickHandler(handler)`

Component props:
- `controller`
- `visible` (bind to `controller.visible` to drive show/hide animation)
- `edgePadding`
- `compactSize`
- `expandedWidth`
- `expandedHeight`
- `animationDurationMs`

Example:
```ts
import { FloatingOrb, FloatingOrbController } from '../ui_components/FloatingOrb';

@State private orbController = FloatingOrbController.create('Task running', 'expanded', () => {});

FloatingOrb({ controller: this.orbController, visible: this.orbController.visible })
```
