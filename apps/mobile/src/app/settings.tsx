import { router } from 'expo-router';
import { ScrollView, StyleSheet, Text, View } from 'react-native';
import { CloudAccountSection } from '@/components/cloud-account-section';
import { useScreenHeaderInset } from '@/components/screen-header';
import { WorkbenchCard, WorkbenchRow } from '@/components/workbench';
import { useTheme } from '@/hooks/use-theme';

export default function SettingsScreen() {
  const theme = useTheme();
  const header = useScreenHeaderInset();
  return <ScrollView style={{ backgroundColor: theme.background }} contentContainerStyle={[styles.content, { paddingTop: header + 12 }]}>
    <WorkbenchCard><CloudAccountSection /></WorkbenchCard>
    <View style={styles.group}>
      <Text accessibilityRole="header" style={[styles.section, { color: theme.textSecondary }]}>Workspace</Text>
      <WorkbenchCard flush>
        <WorkbenchRow label="Paired desktops" icon={{ android: 'devices', web: 'devices' }} onPress={() => router.push('/daemons')} />
        <WorkbenchRow label="Apps" icon={{ android: 'extension', web: 'extension' }} onPress={() => router.push('/apps')} />
        <WorkbenchRow label="Dictation" icon={{ android: 'mic', web: 'mic' }} onPress={() => router.push('/speech')} />
      </WorkbenchCard>
    </View>
    <View style={styles.group}>
      <Text accessibilityRole="header" style={[styles.section, { color: theme.textSecondary }]}>Activity</Text>
      <WorkbenchCard flush>
        <WorkbenchRow label="Usage" icon={{ android: 'bar_chart', web: 'bar_chart' }} onPress={() => router.push('/usage')} />
      </WorkbenchCard>
    </View>
  </ScrollView>;
}
const styles = StyleSheet.create({ content: { width: '100%', maxWidth: 800, alignSelf: 'center', padding: 16, gap: 24, paddingBottom: 32 }, group: { gap: 8 }, section: { fontSize: 14, lineHeight: 20, fontWeight: '600', paddingHorizontal: 4 } });
