import type { AutomationRun } from '@waku/client';
import { StyleSheet, Text, View } from 'react-native';

import { AppSymbol } from './app-symbol';
import type { TicketStatus } from './ticket-card';
import { MonoFont } from '@/constants/theme';
import { useTheme } from '@/hooks/use-theme';
import { automationStatus } from '@/lib/automation-api';

type SymbolName = Parameters<typeof AppSymbol>[0]['name'];
const ICONS: Record<AutomationRun['status'], SymbolName> = {
  running: { ios: 'arrow.triangle.2.circlepath', android: 'progress_activity', web: 'progress_activity' },
  waitingDecision: { ios: 'exclamationmark.circle', android: 'error', web: 'error' },
  blocked: { ios: 'exclamationmark.triangle', android: 'warning', web: 'warning' },
  completed: { ios: 'checkmark', android: 'check', web: 'check' },
  canceled: { ios: 'stop.circle', android: 'block', web: 'block' },
  skipped: { ios: 'forward', android: 'skip_next', web: 'skip_next' },
};
const TONES: Record<AutomationRun['status'], TicketStatus['tone']> = {
  running: 'accent', waitingDecision: 'accent', blocked: 'warning', completed: 'success', canceled: 'neutral', skipped: 'neutral',
};

/** Status pill for a workflow or routine run, labelled like the desktop. */
export function runStatus(status: AutomationRun['status']): TicketStatus {
  return { label: automationStatus(status), icon: ICONS[status], tone: TONES[status] };
}

/** Round status tile leading a run row. */
export function RunStatusIcon({ status }: { status: AutomationRun['status'] }) {
  const theme = useTheme();
  const tone = TONES[status];
  const [color, fill] = tone === 'accent' ? [theme.accent, theme.accentSoft]
    : tone === 'success' ? [theme.success, theme.successSoft]
      : tone === 'warning' ? [theme.warning, theme.warningSoft] : [theme.textSecondary, theme.overlayStrong];
  return (
    <View style={[styles.icon, { backgroundColor: fill }]}>
      <AppSymbol name={ICONS[status]} size={16} tintColor={color} />
    </View>
  );
}

/** One segment per stage: done, current, skipped or still ahead. */
export function StageProgress({ run }: { run: AutomationRun }) {
  const theme = useTheme();
  const done = run.stages.filter(stage => stage.completed).length;
  if (!run.stages.length) return null;
  const live = run.status !== 'completed' && run.status !== 'canceled';
  // Lime only when the current stage needs you; a stage in progress is a
  // lighter stroke of the done color, so it reads apart from a finished one.
  const segmentStyle = (stage: AutomationRun['stages'][number], index: number) =>
    stage.completed ? { backgroundColor: theme.accent }
      : stage.skipped ? { backgroundColor: theme.borderStrong }
        : index === run.currentStage && run.status === 'waitingDecision' ? { backgroundColor: theme.primary }
          : index === run.currentStage && live ? { backgroundColor: theme.accent, opacity: 0.4 }
            : { backgroundColor: theme.overlayStrong };
  return (
    <View accessible accessibilityLabel={`${done} of ${run.stages.length} stages complete`} style={styles.progress}>
      <View style={styles.track}>
        {run.stages.map((stage, index) => (
          <View key={stage.teamRunId || index} style={[styles.segment, segmentStyle(stage, index)]} />
        ))}
      </View>
      <Text style={[styles.count, { color: theme.textTertiary }]}>{done}/{run.stages.length}</Text>
    </View>
  );
}

const styles = StyleSheet.create({
  icon: { alignItems: 'center', borderRadius: 16, height: 32, justifyContent: 'center', width: 32 },
  progress: { alignItems: 'center', flexDirection: 'row', gap: 10 },
  track: { flex: 1, flexDirection: 'row', gap: 3 },
  segment: { borderRadius: 2, flex: 1, height: 4 },
  count: { fontFamily: MonoFont, fontSize: 12, fontVariant: ['tabular-nums'], lineHeight: 16 },
});
