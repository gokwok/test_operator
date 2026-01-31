import { KeyCode } from '@kit.InputKit';

export type KeyCodeInput = number | string;

// KeyCodeResolver
// - Converts "human-friendly" single-key inputs (used by debug UI and tools) into InputKit KeyCode numbers.
// - Notes:
//   1) Single-digit strings like "2" are treated as keyboard digit keys (KEYCODE_2), NOT numeric keycode 2 (KEYCODE_BACK).
//   2) Multi-digit numeric strings like "2054" are NOT treated as keycodes here; pass a number instead if you need it.

const KEYCODE_DIGIT_0: number = 2000;
const KEYCODE_LETTER_A: number = 2017;
const KEYCODE_STAR: number = 2010;
const KEYCODE_POUND: number = 2011;
const KEYCODE_COMMA: number = 2043;
const KEYCODE_PERIOD: number = 2044;

function resolveKeyCodeCore(core: string, hasKeyCodePrefix: boolean): number | null {
  // Fast path for 1-character inputs:
  // - '0'..'9' -> KEYCODE_0..KEYCODE_9
  // - 'A'..'Z' -> KEYCODE_A..KEYCODE_Z
  // - '*', '#', ',', '.' -> corresponding punctuation KeyCodes
  if (core.length === 1) {
    const code = core.charCodeAt(0);
    // 0-9
    if (code >= 48 && code <= 57) {
      return KEYCODE_DIGIT_0 + (code - 48);
    }
    // A-Z
    if (code >= 65 && code <= 90) {
      return KEYCODE_LETTER_A + (code - 65);
    }
    // Common printable symbols
    if (core === '*') {
      return KEYCODE_STAR;
    }
    if (core === '#') {
      return KEYCODE_POUND;
    }
    if (core === ',') {
      return KEYCODE_COMMA;
    }
    if (core === '.') {
      return KEYCODE_PERIOD;
    }
  }

  // Named keys:
  // - `hasKeyCodePrefix` controls ambiguous aliases. Example:
  //   "up" (no prefix) is accepted as DPAD_UP, but "KEYCODE_UP" is rejected (returns null)
  //   because InputKit's canonical name is "KEYCODE_DPAD_UP".
  const mapped: number | null | undefined = (() => {
    switch (core) {
    case 'BACK':
      return KeyCode.KEYCODE_BACK;
    case 'HOME':
      return KeyCode.KEYCODE_HOME;
    case 'MENU':
      return KeyCode.KEYCODE_MENU;
    case 'TASKMANAGER':
      return KeyCode.KEYCODE_TASKMANAGER;
    case 'RECENTS':
    case 'RECENT':
    case 'APP_SWITCH':
      return hasKeyCodePrefix ? null : KeyCode.KEYCODE_TASKMANAGER;
    case 'ENTER':
      return KeyCode.KEYCODE_ENTER;
    case 'DEL':
      return KeyCode.KEYCODE_DEL;
    case 'BACKSPACE':
      return hasKeyCodePrefix ? null : KeyCode.KEYCODE_DEL;
    case 'FORWARD_DEL':
      return KeyCode.KEYCODE_FORWARD_DEL;
    case 'DELETE':
      return hasKeyCodePrefix ? null : KeyCode.KEYCODE_FORWARD_DEL;
    case 'ESCAPE':
      return KeyCode.KEYCODE_ESCAPE;
    case 'ESC':
      return hasKeyCodePrefix ? null : KeyCode.KEYCODE_ESCAPE;
    case 'TAB':
      return KeyCode.KEYCODE_TAB;
    case 'SPACE':
      return KeyCode.KEYCODE_SPACE;
    case 'DPAD_UP':
      return KeyCode.KEYCODE_DPAD_UP;
    case 'UP':
      return hasKeyCodePrefix ? null : KeyCode.KEYCODE_DPAD_UP;
    case 'DPAD_DOWN':
      return KeyCode.KEYCODE_DPAD_DOWN;
    case 'DOWN':
      return hasKeyCodePrefix ? null : KeyCode.KEYCODE_DPAD_DOWN;
    case 'DPAD_LEFT':
      return KeyCode.KEYCODE_DPAD_LEFT;
    case 'LEFT':
      return hasKeyCodePrefix ? null : KeyCode.KEYCODE_DPAD_LEFT;
    case 'DPAD_RIGHT':
      return KeyCode.KEYCODE_DPAD_RIGHT;
    case 'RIGHT':
      return hasKeyCodePrefix ? null : KeyCode.KEYCODE_DPAD_RIGHT;
    case 'VOLUME_UP':
      return KeyCode.KEYCODE_VOLUME_UP;
    case 'VOLUME_DOWN':
      return KeyCode.KEYCODE_VOLUME_DOWN;
    case 'VOLUME_MUTE':
      return KeyCode.KEYCODE_VOLUME_MUTE;
    case 'POWER':
      return KeyCode.KEYCODE_POWER;
    case 'CTRL':
    case 'CONTROL':
      return KeyCode.KEYCODE_CTRL_LEFT;
    case 'CTRL_LEFT':
      return KeyCode.KEYCODE_CTRL_LEFT;
    case 'CTRL_RIGHT':
      return KeyCode.KEYCODE_CTRL_RIGHT;
    case 'SHIFT':
      return KeyCode.KEYCODE_SHIFT_LEFT;
    case 'SHIFT_LEFT':
      return KeyCode.KEYCODE_SHIFT_LEFT;
    case 'SHIFT_RIGHT':
      return KeyCode.KEYCODE_SHIFT_RIGHT;
    case 'ALT':
      return KeyCode.KEYCODE_ALT_LEFT;
    case 'ALT_LEFT':
      return KeyCode.KEYCODE_ALT_LEFT;
    case 'ALT_RIGHT':
      return KeyCode.KEYCODE_ALT_RIGHT;
    case 'META':
    case 'CMD':
    case 'WIN':
      return KeyCode.KEYCODE_META_LEFT;
    case 'META_LEFT':
      return KeyCode.KEYCODE_META_LEFT;
    case 'META_RIGHT':
      return KeyCode.KEYCODE_META_RIGHT;
    case 'COMMA':
      return KEYCODE_COMMA;
    case 'PERIOD':
    case 'DOT':
      return KEYCODE_PERIOD;
    default:
      return undefined;
    }
  })();

  if (mapped !== undefined) {
    return mapped;
  }
  return null;
}

function normalizeKeyCodeToken(raw: string): string {
  // Normalization:
  // - trim whitespace
  // - allow "KeyCode.KEYCODE_BACK" style inputs by taking the last segment after '.'
  // - uppercase
  // - convert spaces and '-' into '_' so "volume up" / "volume-up" match "VOLUME_UP"
  const trimmed = raw.trim();
  if (trimmed.length === 0) {
    return '';
  }
  const token = trimmed.includes('.') ? (trimmed.split('.').pop() ?? trimmed) : trimmed;
  return token.toUpperCase().replace(/[\s-]+/g, '_');
}

export function resolveCommonKeyCode(code: KeyCodeInput): number | null {
  // Resolution order:
  // 1) number -> treat as already a keycode (truncate to int)
  // 2) string token:
  //    - single digit "0".."9" -> KEYCODE_0..KEYCODE_9 (keyboard digit key)
  //    - otherwise normalize and resolve:
  //      - "KEYCODE_XXX" uses the explicit prefix
  //      - "xxx" uses friendly aliases and canonical names
  if (typeof code === 'number' && Number.isFinite(code)) {
    return Math.trunc(code);
  }

  const raw = String(code);
  const token = raw.trim().includes('.') ? (raw.trim().split('.').pop() ?? raw.trim()) : raw.trim();
  if (token.length === 0) {
    return null;
  }
  if (/^\d$/.test(token)) {
    return KEYCODE_DIGIT_0 + Number.parseInt(token, 10);
  }

  const normalized = normalizeKeyCodeToken(token);
  if (normalized.length === 0) {
    return null;
  }

  const hasKeyCodePrefix = normalized.startsWith('KEYCODE_');
  const core = hasKeyCodePrefix ? normalized.substring('KEYCODE_'.length) : normalized;
  return resolveKeyCodeCore(core, hasKeyCodePrefix);
}
