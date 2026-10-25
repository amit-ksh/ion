import { View } from 'react-native';
import { SvgXml } from 'react-native-svg';

import { useTheme } from '@/hooks/use-theme';

/** The canonical mark from `assets/icons/ion.svg`, tinted by the host. */
const ION_MARK = '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><circle cx="12" cy="4.75" r="2.75" fill="#000"/><polygon points="8.6,9.3 15.4,9.3 15.4,21.8 13.7,20.2 12,21.8 10.3,20.2 8.6,21.8" fill="#000"/></svg>';

/** Brand tile: lime with the dark-ink mark. */
export function IonMarkTile({ size = 56 }: { size?: number }) {
  const theme = useTheme();
  const mark = Math.round(size * 0.62);
  return (
    <View accessibilityLabel="Ion" accessibilityRole="image"
      style={{ alignItems: 'center', backgroundColor: theme.primary, borderRadius: Math.round(size * 0.28), height: size, justifyContent: 'center', width: size }}>
      <SvgXml height={mark} width={mark} xml={ION_MARK.replaceAll('#000', theme.onPrimary)} />
    </View>
  );
}
