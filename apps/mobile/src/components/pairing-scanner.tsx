import { CameraView, useCameraPermissions } from 'expo-camera';
import { useEffect, useRef, useState } from 'react';
import {
  AppState,
  Linking,
  Modal,
  Pressable,
  StyleSheet,
  Text,
  View,
} from 'react-native';
import { SafeAreaView } from 'react-native-safe-area-context';
import { AppSymbol } from '@/components/app-symbol';
import { useTheme } from '@/hooks/use-theme';
import { useReducedMotion } from '@/hooks/use-reduced-motion';
import { parseDaemonPairingCode } from '@/lib/daemon-pairing';
import type { DaemonProfileInput } from '@/lib/daemon-profile';

export function PairingScanButton({
  disabled,
  onScan,
}: {
  disabled?: boolean;
  onScan: (input: Required<DaemonProfileInput>) => void;
}) {
  const theme = useTheme();
  const [open, setOpen] = useState(false);
  return (
    <>
      <Pressable
        accessibilityRole="button"
        accessibilityState={{ disabled: !!disabled }}
        disabled={disabled}
        onPress={() => setOpen(true)}
        style={({ pressed }) => [
          styles.scanButton,
          {
            backgroundColor: theme.overlay,
            opacity: disabled ? 0.4 : pressed ? 0.65 : 1,
          },
        ]}
      >
        <AppSymbol
          name={{
            ios: 'qrcode.viewfinder',
            android: 'qr_code_scanner',
            web: 'qr_code_scanner',
          }}
          size={22}
          tintColor={theme.accent}
        />
        <Text style={[styles.buttonText, { color: theme.text }]}>
          Scan to pair
        </Text>
      </Pressable>
      {open && (
        <PairingScanner
          onClose={() => setOpen(false)}
          onScan={(input) => {
            setOpen(false);
            onScan(input);
          }}
        />
      )}
    </>
  );
}

function PairingScanner({
  onClose,
  onScan,
}: {
  onClose: () => void;
  onScan: (input: Required<DaemonProfileInput>) => void;
}) {
  const theme = useTheme();
  const reducedMotion = useReducedMotion();
  const [permission, requestPermission, refreshPermission] = useCameraPermissions();
  const [error, setError] = useState<string | null>(null);
  const [active, setActive] = useState(AppState.currentState === 'active');
  const consumed = useRef(false);
  useEffect(() => {
    const subscription = AppState.addEventListener('change', (state) => {
      setActive(state === 'active');
      // Settings can change a previously denied permission while this modal stays open.
      if (state === 'active') void refreshPermission().catch(() => {});
    });
    return () => subscription.remove();
  }, [refreshPermission]);
  function scan(data: string) {
    if (consumed.current || !active) return;
    consumed.current = true;
    try {
      onScan(parseDaemonPairingCode(data));
    } catch (cause) {
      setError(
        cause instanceof Error
          ? cause.message
          : 'Could not read this pairing code.',
      );
    }
  }
  return (
    <Modal
      animationType={reducedMotion ? 'none' : 'slide'}
      onRequestClose={onClose}
      presentationStyle="fullScreen"
    >
      <SafeAreaView
        style={[styles.screen, { backgroundColor: theme.background }]}
      >
        <View style={styles.header}>
          <Text
            accessibilityRole="header"
            style={[styles.title, { color: theme.text }]}
          >
            Scan to pair
          </Text>
          <Pressable
            accessibilityRole="button"
            onPress={onClose}
            style={styles.action}
          >
            <Text style={[styles.buttonText, { color: theme.accent }]}>
              Cancel
            </Text>
          </Pressable>
        </View>
        <Text style={[styles.description, { color: theme.textSecondary }]}>
          Scan your desktop’s Ion pairing QR code. Review the address, then tap
          Add to connect.
        </Text>
        {permission?.granted ? (
          <View style={styles.cameraFrame}>
            {active && !error && (
              <CameraView
                style={StyleSheet.absoluteFill}
                facing="back"
                barcodeScannerSettings={{ barcodeTypes: ['qr'] }}
                onBarcodeScanned={({ data }) => scan(data)}
                onMountError={() =>
                  setError(
                    'Could not open the camera. You can enter the connection details manually.',
                  )
                }
              />
            )}
            {!error && <View pointerEvents="none" style={styles.viewfinder} />}
          </View>
        ) : (
          <View style={styles.permission}>
            <AppSymbol
              name={{
                ios: 'camera',
                android: 'photo_camera',
                web: 'photo_camera',
              }}
              size={42}
              tintColor={theme.textSecondary}
            />
            <Text style={[styles.description, { color: theme.textSecondary }]}>
              Allow camera access to scan a pairing code, or cancel to enter the
              details manually.
            </Text>
            {permission && (
              <Pressable
                accessibilityRole="button"
                style={[styles.primary, { backgroundColor: theme.primary }]}
                onPress={() => {
                  void (
                    permission.canAskAgain
                      ? requestPermission()
                      : Linking.openSettings()
                  ).catch(() =>
                    setError(
                      'Camera access is unavailable. Enter the details manually.',
                    ),
                  );
                }}
              >
                <Text style={[styles.buttonText, { color: theme.onPrimary }]}>
                  {permission.canAskAgain ? 'Allow camera' : 'Open settings'}
                </Text>
              </Pressable>
            )}
          </View>
        )}
        {error && (
          <View style={styles.feedback}>
            <Text
              accessibilityLiveRegion="polite"
              style={[styles.description, { color: theme.danger }]}
            >
              {error}
            </Text>
            <Pressable
              accessibilityRole="button"
              style={styles.action}
              onPress={() => {
                consumed.current = false;
                setError(null);
              }}
            >
              <Text style={[styles.buttonText, { color: theme.accent }]}>
                Try again
              </Text>
            </Pressable>
          </View>
        )}
      </SafeAreaView>
    </Modal>
  );
}

const styles = StyleSheet.create({
  screen: { flex: 1, padding: 20 },
  header: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
  },
  title: { fontSize: 24, fontWeight: '700', flexShrink: 1 },
  action: {
    minHeight: 48,
    minWidth: 48,
    alignItems: 'center',
    justifyContent: 'center',
    paddingHorizontal: 8,
  },
  buttonText: { fontSize: 16, fontWeight: '600' },
  description: { fontSize: 15, lineHeight: 23, marginVertical: 16 },
  scanButton: {
    flexDirection: 'row',
    gap: 10,
    alignItems: 'center',
    justifyContent: 'center',
    minHeight: 52,
    borderRadius: 14,
    marginHorizontal: 16,
    marginBottom: 20,
    padding: 12,
  },
  cameraFrame: {
    flex: 1,
    minHeight: 180,
    maxHeight: 480,
    borderRadius: 24,
    overflow: 'hidden',
    backgroundColor: '#101411',
    alignItems: 'center',
    justifyContent: 'center',
  },
  viewfinder: {
    width: '76%',
    aspectRatio: 1,
    maxHeight: 280,
    borderWidth: 2,
    borderColor: '#c2ef68',
    borderRadius: 20,
  },
  permission: {
    flex: 1,
    alignItems: 'center',
    justifyContent: 'center',
    padding: 16,
  },
  primary: {
    minHeight: 52,
    paddingHorizontal: 24,
    justifyContent: 'center',
    borderRadius: 14,
  },
  feedback: { paddingHorizontal: 8 },
});
