import { Platform, PlatformColor, type ColorValue } from 'react-native';

/**
 * Ion's workbench palette, mirrored from the desktop theme (src/theme.rs) so
 * every client reads as one product. Layer tokens follow the desktop naming:
 * background (canvas) → surface (raised cards) → surfaceMuted (fills on cards)
 * → inset (code / terminal wells), with hairline `border` strokes. Tickets,
 * the composer and floating layers add the soft `elevation*` shadows.
 */
export const Colors = {
  light: {
    text: '#121712',
    textSecondary: '#465045',
    textTertiary: '#5a6457',
    textGhost: '#626c5f',
    background: '#f6f7f5',
    surface: '#fbfcfa',
    surfaceMuted: '#f1f3ef',
    raised: '#ffffff',
    inset: '#f1f3ef',
    composer: '#ffffff',
    backgroundElement: '#eef1ec',
    backgroundSelected: '#e9f5d2',
    overlay: 'hsla(150, 8%, 12%, 0.05)',
    overlayStrong: 'hsla(150, 8%, 12%, 0.085)',
    separator: 'rgba(18, 23, 18, 0.08)',
    border: 'hsla(150, 8%, 12%, 0.08)',
    borderStrong: 'hsla(150, 8%, 12%, 0.14)',
    accent: '#3d5a12',
    accentSoft: 'rgba(150, 205, 60, 0.18)',
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
    shadow: 'rgba(18, 32, 22, 0.10)',
    /** Home glow field and live-work tickets, mirrored from desktop. */
    glowMint: 'rgba(181, 236, 214, 0.85)',
    glowLime: 'rgba(214, 245, 160, 0.55)',
    ticket: '#ffffff',
    ticketBorder: 'hsla(150, 8%, 12%, 0.09)',
    /** Layered soft shadows mirroring desktop `elevation()` (contact + ambient). */
    elevationRest: '0px 1px 2px rgba(18, 32, 22, 0.09), 0px 6px 18px -4px rgba(18, 32, 22, 0.07)',
    elevationFloat: '0px 4px 10px rgba(18, 32, 22, 0.10), 0px 18px 44px -4px rgba(18, 32, 22, 0.13)',
  },
  dark: {
    text: '#eef2ec',
    textSecondary: '#bac2b7',
    textTertiary: '#9da69a',
    textGhost: '#8a9487',
    background: '#0f1211',
    surface: '#121615',
    surfaceMuted: '#1a1f1d',
    raised: '#1a1f1d',
    inset: '#0d100f',
    composer: '#171c1a',
    backgroundElement: '#1a1f1d',
    backgroundSelected: '#26301d',
    overlay: 'hsla(150, 10%, 90%, 0.05)',
    overlayStrong: 'hsla(150, 10%, 90%, 0.09)',
    separator: 'rgba(230, 236, 230, 0.08)',
    border: 'hsla(150, 10%, 90%, 0.075)',
    borderStrong: 'hsla(150, 10%, 90%, 0.14)',
    accent: '#c2ef68',
    accentSoft: 'rgba(194, 239, 104, 0.13)',
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
    glowMint: 'rgba(48, 166, 128, 0.20)',
    glowLime: 'rgba(160, 222, 70, 0.12)',
    ticket: '#181d1b',
    ticketBorder: 'hsla(150, 10%, 90%, 0.09)',
    elevationRest: '0px 1px 2px rgba(0, 0, 0, 0.40), 0px 6px 18px -4px rgba(0, 0, 0, 0.32)',
    elevationFloat: '0px 4px 10px rgba(0, 0, 0, 0.45), 0px 18px 44px -4px rgba(0, 0, 0, 0.58)',
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
