import { useState } from 'react';
import { router } from 'expo-router';

import { DaemonOnboarding } from '@/components/daemon-onboarding';
import { DaemonPickerSheet } from '@/components/daemon-picker-sheet';
import { useDaemon } from '@/lib/daemon-context';

import NewTaskScreen from './new-task';

export default function HomeScreen() {
  const daemon = useDaemon();
  const [daemonPickerOpen, setDaemonPickerOpen] = useState(false);

  if (daemon.phase === 'booting') return null;

  return (
    <>
      {daemon.profiles.length ? (
        <NewTaskScreen />
      ) : (
        <DaemonOnboarding
          onAddDaemon={() => setDaemonPickerOpen(true)}
          onPairGateway={() => router.push('/gateway')}
        />
      )}
      {(!daemon.profiles.length || daemonPickerOpen) && (
        <DaemonPickerSheet
          onDismiss={() => setDaemonPickerOpen(false)}
          visible={daemonPickerOpen}
        />
      )}
    </>
  );
}
