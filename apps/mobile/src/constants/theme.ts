import { Platform, PlatformColor, type ColorValue } from 'react-native';

/**
 * Ion's workbench palette, mirrored from the desktop theme (src/theme.rs) so
 * every client reads as one product. Layer tokens follow the desktop naming:
 * background (canvas) → surface (raised cards) → surfaceMuted (fills on cards)
 * → inset (code / terminal wells), with hairline `border` strokes instead of
 * shadows.
 */
export const Colors = {
  light: {
    text: '#20271f',
    textSecondary: '#505b4d',
    textTertiary: '#606b5c',
    textGhost: '#626d5e',
    background: '#f5f6f2',
    surface: '#fafbf8',
    surfaceMuted: '#ecefe8',
    raised: '#ffffff',
    inset: '#ebeee7',
    composer: '#ffffff',
    backgroundElement: '#ecefe8',
    backgroundSelected: '#e4ebd8',
    overlay: 'hsla(220, 10%, 12%, 0.05)',
    overlayStrong: 'hsla(220, 10%, 12%, 0.09)',
    separator: 'rgba(28, 31, 37, 0.10)',
    border: 'hsla(220, 10%, 12%, 0.08)',
    borderStrong: 'hsla(220, 10%, 12%, 0.15)',
    accent: '#47651b',
    accentSoft: 'rgba(100, 135, 39, 0.10)',
    primary: '#c2ef68',
    onPrimary: '#1b2510',
    codeText: '#9a5528',
    codeWash: 'hsla(220, 10%, 12%, 0.07)',
    inverse: '#202227',
    onInverse: '#f8f8f9',
    success: '#237640',
    successSoft: 'rgba(47, 143, 82, 0.12)',
    warning: '#875014',
    warningSoft: 'rgba(166, 107, 32, 0.12)',
    danger: '#ac3933',
    dangerSoft: 'hsla(4, 55%, 52%, 0.10)',
    shadow: 'rgba(0, 0, 0, 0.12)',
  },
  dark: {
    text: '#f2f4ef',
    textSecondary: '#c2c8c0',
    textTertiary: '#abb3a8',
    textGhost: '#9aa395',
    background: '#181b19',
    surface: '#1d201e',
    surfaceMuted: '#272b28',
    raised: '#272b28',
    inset: '#131614',
    composer: '#222623',
    backgroundElement: '#272b28',
    backgroundSelected: '#303a25',
    overlay: 'hsla(220, 10%, 90%, 0.05)',
    overlayStrong: 'hsla(220, 10%, 90%, 0.09)',
    separator: 'rgba(230, 230, 230, 0.09)',
    border: 'hsla(220, 10%, 90%, 0.07)',
    borderStrong: 'hsla(220, 10%, 90%, 0.14)',
    accent: '#c2ef68',
    accentSoft: 'rgba(194, 239, 104, 0.12)',
    primary: '#c2ef68',
    onPrimary: '#1b2510',
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

/** Platform selection tint. Filled brand actions use primary/onPrimary. */
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
  large: 16,
  pill: 999,
} as const;

export const BottomTabInset = Platform.select({ ios: 50, android: 80 }) ?? 0;
export const MaxContentWidth = 800;
