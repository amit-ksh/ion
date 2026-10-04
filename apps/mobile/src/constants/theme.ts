import { Platform, PlatformColor, type ColorValue } from 'react-native';

/**
 * Waku's graphite palette, mirrored from the desktop theme (src/theme.rs) so
 * every client reads as one product. Layer tokens follow the desktop naming:
 * background (canvas) → surface (raised cards) → surfaceMuted (fills on cards)
 * → inset (code / terminal wells), with hairline `border` strokes instead of
 * shadows.
 */
export const Colors = {
  light: {
    text: '#16251a',
    textSecondary: '#465c4a',
    textTertiary: '#526b56',
    textGhost: '#627766',
    background: '#eff5ee',
    surface: '#eff5ee',
    surfaceMuted: '#e2ecdf',
    raised: '#e2ecdf',
    inset: '#dae5d7',
    composer: '#eff5ee',
    backgroundElement: '#e2ecdf',
    backgroundSelected: '#e2e1e2',
    overlay: 'hsla(220, 10%, 12%, 0.05)',
    overlayStrong: 'hsla(220, 10%, 12%, 0.09)',
    separator: 'rgba(28, 31, 37, 0.10)',
    border: 'hsla(220, 10%, 12%, 0.08)',
    borderStrong: 'hsla(220, 10%, 12%, 0.15)',
    accent: '#38621b',
    accentSoft: 'rgba(200, 95, 68, 0.12)',
    codeText: '#9a5528',
    codeWash: 'hsla(220, 10%, 12%, 0.07)',
    inverse: '#202227',
    onInverse: '#f8f8f9',
    success: '#2f8f52',
    successSoft: 'rgba(47, 143, 82, 0.12)',
    warning: '#a66b20',
    warningSoft: 'rgba(166, 107, 32, 0.12)',
    danger: '#c64a42',
    dangerSoft: 'hsla(4, 55%, 52%, 0.10)',
    shadow: 'rgba(0, 0, 0, 0.12)',
  },
  dark: {
    text: '#e2e2e2',
    textSecondary: '#a3a3a3',
    textTertiary: '#7d7d7d',
    textGhost: '#575757',
    background: '#1a1a1a',
    surface: '#20332a',
    surfaceMuted: '#2a2a2a',
    raised: '#20332a',
    inset: '#111c17',
    composer: '#1d2e25',
    backgroundElement: '#20332a',
    backgroundSelected: '#303030',
    overlay: 'hsla(220, 10%, 90%, 0.05)',
    overlayStrong: 'hsla(220, 10%, 90%, 0.09)',
    separator: 'rgba(230, 230, 230, 0.09)',
    border: 'hsla(220, 10%, 90%, 0.07)',
    borderStrong: 'hsla(220, 10%, 90%, 0.14)',
    accent: '#b9f36b',
    accentSoft: 'rgba(226, 121, 91, 0.15)',
    codeText: '#e0a882',
    codeWash: 'hsla(220, 10%, 90%, 0.08)',
    inverse: '#e7e9ec',
    onInverse: '#17181c',
    success: '#62c987',
    successSoft: 'rgba(98, 201, 135, 0.14)',
    warning: '#e0b36a',
    warningSoft: 'rgba(224, 179, 106, 0.14)',
    danger: '#e2726a',
    dangerSoft: 'hsla(4, 55%, 63%, 0.10)',
    shadow: 'rgba(0, 0, 0, 0.45)',
  },
} as const;

export type ThemeColor = keyof typeof Colors.light & keyof typeof Colors.dark;

/** System tint for interactive affordances (back rows, checkmarks, active
 * toggles, text carets) so controls read native instead of branded. */
export const NativeTint: ColorValue =
  Platform.OS === 'ios' ? PlatformColor('systemBlue') : '#3b82f6';

export const Fonts = Platform.select({
  ios: {
    /** iOS `UIFontDescriptorSystemDesignDefault` */
    sans: 'system-ui',
    /** iOS `UIFontDescriptorSystemDesignSerif` */
    serif: 'ui-serif',
    /** iOS `UIFontDescriptorSystemDesignRounded` */
    rounded: 'ui-rounded',
    /** iOS `UIFontDescriptorSystemDesignMonospaced` */
    mono: 'ui-monospace',
  },
  default: {
    sans: 'normal',
    serif: 'serif',
    rounded: 'normal',
    mono: 'monospace',
  },
  web: {
    sans: 'var(--font-display)',
    serif: 'var(--font-serif)',
    rounded: 'var(--font-rounded)',
    mono: 'var(--font-mono)',
  },
});

export const MonoFont = Platform.select({
  ios: 'Menlo',
  android: 'monospace',
  default: 'monospace',
});

export const Spacing = {
  half: 2,
  one: 4,
  two: 8,
  three: 16,
  four: 24,
  five: 32,
  six: 64,
} as const;

export const Radius = {
  tiny: 6,
  small: 8,
  medium: 12,
  large: 18,
  pill: 999,
} as const;

export const BottomTabInset = Platform.select({ ios: 50, android: 80 }) ?? 0;
export const MaxContentWidth = 800;
