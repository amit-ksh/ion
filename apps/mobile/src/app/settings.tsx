import { router } from 'expo-router';
import { ScrollView, StyleSheet, Text, View } from 'react-native';
import { CloudAccountSection } from '@/components/cloud-account-section';
import { useScreenHeaderInset } from '@/components/screen-header';
import { WorkbenchCard, WorkbenchRow } from '@/components/workbench';
import { useTheme } from '@/hooks/use-theme';

export default function SettingsScreen() {
  const theme = useTheme();
  const header = useScreenHeaderInset();
  return <ScrollView style={{ backgroundColor: theme.background }} contentContainerStyle={[styles.content, { paddingTop: header + 16 }]}>
    <Text accessibilityRole="header" style={[styles.title, { color: theme.text }]}>Settings</Text>
    <WorkbenchCard><CloudAccountSection /></WorkbenchCard>
    <View style={styles.group}>
      <Text accessibilityRole="header" style={[styles.section, { color: theme.text }]}>Workspace</Text>
      <WorkbenchCard>
        <WorkbenchRow label="Paired desktops" detail="Connections and host selection" icon={{ android: 'devices', web: 'devices' }} onPress={() => router.push('/daemons')} />
        <WorkbenchRow label="Apps" detail="Connected accounts and app access" icon={{ android: 'extension', web: 'extension' }} onPress={() => router.push('/apps')} />
        <WorkbenchRow label="Dictation" detail="Local Parakeet speech on your desktop" icon={{ android: 'mic', web: 'mic' }} onPress={() => router.push('/speech')} />
      </WorkbenchCard>
    </View>
    <View style={styles.group}>
      <Text accessibilityRole="header" style={[styles.section, { color: theme.text }]}>Usage & resources</Text>
      <WorkbenchCard>
        <WorkbenchRow label="Usage" detail="Tokens and task activity" icon={{ android: 'bar_chart', web: 'bar_chart' }} onPress={() => router.push('/usage')} />
        <WorkbenchRow label="GPU budgets" icon={{ android: 'account_balance_wallet', web: 'account_balance_wallet' }} onPress={() => router.push('/cloud-budgets')} />
        <WorkbenchRow label="Cloud billing" icon={{ android: 'receipt_long', web: 'receipt_long' }} onPress={() => router.push('/cloud-billing')} />
        <WorkbenchRow label="Remote executions" icon={{ android: 'dns', web: 'dns' }} onPress={() => router.push('/remote-executions')} />
      </WorkbenchCard>
    </View>
  </ScrollView>;
}
const styles = StyleSheet.create({ content: { width: '100%', maxWidth: 800, alignSelf: 'center', padding: 16, gap: 24, paddingBottom: 32 }, title: { fontSize: 26, lineHeight: 34, fontWeight: '600' }, group: { gap: 10 }, section: { fontSize: 17, lineHeight: 24, fontWeight: '600' } });
