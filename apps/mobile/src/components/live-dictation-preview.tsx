import { Text, View } from 'react-native';
import { useTheme } from '@/hooks/use-theme';

export function LiveDictationPreview({ text }: { text: string | null }) {
  const theme = useTheme();
  if (!text) return null;
  return (
    <View style={{ paddingHorizontal: 12, paddingVertical: 8 }}>
      <Text
        style={{ color: theme.textSecondary, fontSize: 11, marginBottom: 4 }}
      >
        Live transcript
      </Text>
      <Text
        accessibilityLiveRegion="polite"
        numberOfLines={4}
        style={{ color: theme.text, fontSize: 14, lineHeight: 20 }}
      >
        {text}
      </Text>
    </View>
  );
}
