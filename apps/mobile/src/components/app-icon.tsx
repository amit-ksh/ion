import { useMemo } from 'react';
import { Image, View } from 'react-native';
import { SvgXml } from 'react-native-svg';
import { APP_ICON_ASSETS } from './app-icon-assets';
import { AppSymbol } from './app-symbol';
import { useTheme } from '@/hooks/use-theme';

type AndroidSymbol = NonNullable<Exclude<Parameters<typeof AppSymbol>[0]['name'], string>['android']>;
function fallbackSymbol(toolkit: string): AndroidSymbol {
  return ({ slack: 'forum', discord: 'forum', notion: 'article', googledocs: 'description',
    googlesheets: 'table_chart', googlecalendar: 'calendar_month', linear: 'checklist',
    trello: 'view_kanban' } as Record<string, AndroidSymbol>)[toolkit] ?? 'extension';
}

/** Decorative identity; the adjacent app name and connection state remain visible. */
export function AppIcon({ toolkit, size = 26 }: { toolkit: string; size?: number }) {
  const theme = useTheme();
  const asset = APP_ICON_ASSETS[toolkit];
  const xml = useMemo(() => asset?.kind === 'svg'
    ? asset.monochrome ? asset.xml.replace('fill="#181717"', `fill="${theme.text}"`) : asset.xml
    : null, [asset, theme.text]);
  return <View accessibilityElementsHidden importantForAccessibility="no-hide-descendants" pointerEvents="none"
    style={{ width: size, height: size, alignItems: 'center', justifyContent: 'center' }}>
    {asset?.kind === 'image' ? <Image source={asset.source} resizeMode="contain" style={{ width: size, height: size }} />
      : xml ? <SvgXml xml={xml} width={size} height={size} />
        : <AppSymbol name={{ android: fallbackSymbol(toolkit), web: fallbackSymbol(toolkit) }} size={size} tintColor={theme.textSecondary} />}
  </View>;
}
