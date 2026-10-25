import { useMemo, useState } from 'react';
import { Image, StyleSheet, View } from 'react-native';
import { SvgXml } from 'react-native-svg';
import { APP_ICON_ASSETS } from './app-icon-assets';
import { AppSymbol } from './app-symbol';
import { useTheme } from '@/hooks/use-theme';
import { appLogoUrl } from '@/lib/app-logo';

type AndroidSymbol = NonNullable<Exclude<Parameters<typeof AppSymbol>[0]['name'], string>['android']>;
function fallbackSymbol(toolkit: string): AndroidSymbol {
  return ({ slack: 'forum', discord: 'forum', notion: 'article', googledocs: 'description',
    googlesheets: 'table_chart', googlecalendar: 'calendar_month', linear: 'checklist',
    trello: 'view_kanban' } as Record<string, AndroidSymbol>)[toolkit] ?? 'extension';
}

/** Decorative identity; the adjacent app name and connection state remain visible.
 * Bundled product marks win; any other app shows its company logo, with the
 * generic glyph until it loads and whenever it cannot. */
export function AppIcon({ toolkit, size = 26 }: { toolkit: string; size?: number }) {
  const theme = useTheme();
  const asset = APP_ICON_ASSETS[toolkit];
  const xml = useMemo(() => asset?.kind === 'svg'
    // Monochrome marks carry one root fill; draw it in theme ink.
    ? asset.monochrome ? asset.xml.replace(/fill="#[0-9a-f]{6}"/i, `fill="${theme.text}"`) : asset.xml
    : null, [asset, theme.text]);
  const logo = asset ? null : appLogoUrl(toolkit);
  const [loaded, setLoaded] = useState<{ toolkit: string; ok: boolean } | null>(null);
  const status = loaded?.toolkit === toolkit ? loaded.ok : null;
  return <View accessibilityElementsHidden importantForAccessibility="no-hide-descendants" pointerEvents="none"
    style={{ width: size, height: size, alignItems: 'center', justifyContent: 'center' }}>
    {asset?.kind === 'image' ? <Image source={asset.source} resizeMode="contain" style={{ width: size, height: size }} />
      : xml ? <SvgXml xml={xml} width={size} height={size} />
        : <>
          {status !== true && <AppSymbol name={{ android: fallbackSymbol(toolkit), web: fallbackSymbol(toolkit) }} size={size} tintColor={theme.textSecondary} />}
          {/* Android's image pipeline keeps fetched logos in its disk cache. */}
          {logo && status !== false && <Image
            onError={() => setLoaded({ toolkit, ok: false })}
            onLoad={() => setLoaded({ toolkit, ok: true })}
            resizeMode="cover"
            source={{ uri: logo }}
            style={[StyleSheet.absoluteFill, { borderRadius: size * 0.22, opacity: status ? 1 : 0 }]}
          />}
        </>}
  </View>;
}
