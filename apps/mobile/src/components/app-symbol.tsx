import { SymbolView, type SymbolViewProps } from 'expo-symbols';
import { Platform, StyleSheet, View, useWindowDimensions, type StyleProp, type ViewStyle } from 'react-native';

interface AppSymbolProps extends Omit<SymbolViewProps, 'size' | 'style'> {
  size?: number;
  style?: StyleProp<ViewStyle>;
}

export function AppSymbol({ size = 18, style, ...props }: AppSymbolProps) {
  const { fontScale } = useWindowDimensions();
  // On Android expo-symbols draws the glyph as scalable text, so a larger
  // system font size would grow it past its frame and clip it. Icons keep
  // their designed size; the labels beside them carry the scaling.
  const glyph = Platform.OS === 'android' && fontScale > 0 ? size / fontScale : size;
  return (
    <View style={[styles.frame, { width: size, height: size }, style]}>
      <SymbolView resizeMode="scaleAspectFit" size={glyph} style={{ width: size, height: size }} {...props} />
    </View>
  );
}

const styles = StyleSheet.create({
  frame: {
    alignItems: 'center',
    justifyContent: 'center',
  },
});
