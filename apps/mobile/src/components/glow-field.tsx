import { StyleSheet, View } from 'react-native';

import { useTheme } from '@/hooks/use-theme';

/** The same hue at zero alpha, so the fade never passes through grey. */
const clear = (color: string) => color.replace(/[\d.]+\)$/, '0)');

/**
 * The mint-to-lime glow rising from the bottom edge behind home and the
 * entry screens: two vertical fades, mint taller and lime lower, as on desktop.
 */
export function GlowField() {
  const theme = useTheme();
  return (
    <View pointerEvents="none" style={StyleSheet.absoluteFill}>
      <View style={[styles.layer, { height: '62%', experimental_backgroundImage: `linear-gradient(180deg, ${clear(theme.glowMint)} 0%, ${theme.glowMint} 100%)` }]} />
      <View style={[styles.layer, { height: '30%', experimental_backgroundImage: `linear-gradient(180deg, ${clear(theme.glowLime)} 0%, ${theme.glowLime} 100%)` }]} />
    </View>
  );
}

const styles = StyleSheet.create({
  layer: { bottom: 0, left: 0, position: 'absolute', right: 0 },
});
