import type { WakuClient } from '@waku/client';
import { useEffect } from 'react';
import {
  ActivityIndicator,
  Alert,
  Pressable,
  StyleSheet,
  Text,
  View,
} from 'react-native';

import { AppSymbol } from './app-symbol';
import { usePhoneDictation } from '@/hooks/use-phone-dictation';
import { useTheme } from '@/hooks/use-theme';

export function PhoneDictationControl({
  client,
  contextKey,
  host,
  disabled,
  onTranscript,
  onPreview,
  onError,
}: {
  client: WakuClient | null;
  contextKey: string;
  host: string;
  disabled: boolean;
  onTranscript: (text: string) => void;
  onPreview?: (text: string | null) => void;
  onError: (message: string) => void;
}) {
  const theme = useTheme();
  const dictation = usePhoneDictation(
    client,
    contextKey,
    onTranscript,
    onPreview,
  );
  useEffect(() => {
    if (dictation.error) onError(dictation.error);
  }, [dictation.error, onError]);
  useEffect(() => {
    if (disabled) dictation.discard();
  }, [disabled]);
  const recording = dictation.phase === 'recording';
  const preparing = dictation.phase === 'preparing';
  const transcribing = dictation.phase === 'transcribing';
  function press() {
    if (recording) {
      void dictation
        .stop()
        .catch((cause) =>
          onError(cause instanceof Error ? cause.message : String(cause)),
        );
      return;
    }
    Alert.alert(
      'Record on this phone?',
      `ion streams up to 60 seconds of microphone audio to your configured speech service. Without one, audio is sent to ${host} after you stop. You can review and edit the text before sending it to an agent.`,
      [
        { text: 'Cancel', style: 'cancel' },
        {
          text: 'Start recording',
          onPress: () =>
            void dictation
              .start()
              .catch((cause) =>
                onError(cause instanceof Error ? cause.message : String(cause)),
              ),
        },
      ],
    );
  }
  return (
    <View style={styles.group}>
      <Pressable
        accessibilityRole="button"
        accessibilityLabel={
          recording
            ? 'Stop recording and finalize transcript'
            : preparing
              ? 'Preparing microphone'
              : transcribing
                ? 'Finalizing transcription'
                : 'Dictate'
        }
        accessibilityState={{
          disabled: (!recording && disabled) || preparing || transcribing,
        }}
        disabled={
          (!recording && disabled) ||
          preparing ||
          transcribing ||
          !dictation.available
        }
        onPress={press}
        hitSlop={4}
        style={({ pressed }) => [
          styles.button,
          { opacity: pressed ? 0.55 : 1 },
        ]}
      >
        {preparing || transcribing ? (
          <ActivityIndicator size="small" color={theme.textSecondary} />
        ) : (
          <AppSymbol
            name={{
              ios: recording ? 'stop.circle.fill' : 'mic',
              android: recording ? 'stop_circle' : 'mic',
              web: 'mic',
            }}
            size={20}
            tintColor={recording ? theme.danger : theme.textSecondary}
          />
        )}
      </Pressable>
      {(recording || transcribing || preparing) && (
        <>
          <Text
            accessibilityLiveRegion="polite"
            style={[styles.state, { color: theme.danger }]}
          >
            {recording ? (dictation.cloud ? 'Streaming' : 'Recording') : ''}
          </Text>
          <Pressable
            accessibilityLabel="Discard recording"
            accessibilityRole="button"
            onPress={dictation.discard}
            style={styles.discard}
            hitSlop={4}
          >
            <AppSymbol
              name={{ ios: 'xmark', android: 'close', web: 'close' }}
              size={13}
              tintColor={theme.textSecondary}
            />
          </Pressable>
        </>
      )}
      {preparing && (
        <Text
          accessibilityLiveRegion="polite"
          style={[styles.state, { color: theme.textSecondary }]}
        >
          Preparing…
        </Text>
      )}
      {transcribing && (
        <Text
          accessibilityLiveRegion="polite"
          style={[styles.state, { color: theme.textSecondary }]}
        >
          Finalizing…
        </Text>
      )}
    </View>
  );
}

const styles = StyleSheet.create({
  group: { flexDirection: 'row', alignItems: 'center' },
  button: {
    minWidth: 44,
    minHeight: 44,
    alignItems: 'center',
    justifyContent: 'center',
  },
  state: { fontSize: 10, fontWeight: '600' },
  discard: {
    minWidth: 44,
    minHeight: 44,
    alignItems: 'center',
    justifyContent: 'center',
  },
});
