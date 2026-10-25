import { useState } from 'react';

import { Chip } from './chip';
import { ComposerIconButton } from './composer-icon-button';
import type { ComposerAccessMenuProps } from './composer-access-menu.types';
import { AccessSheet } from './session-option-sheets';
import { runtimeModeLabel } from '@/lib/session-presentation';

const MODE_ICONS = {
  ask: { ios: 'lock', android: 'lock', web: 'lock' },
  autoAcceptEdits: { ios: 'pencil', android: 'edit', web: 'edit' },
  auto: { ios: 'sparkles', android: 'auto_awesome', web: 'auto_awesome' },
  fullAccess: { ios: 'lock.open', android: 'lock_open', web: 'lock_open' },
} as const;

/** Non-iOS fallback. iOS replaces this with a native anchored menu. */
export function ComposerAccessMenu({ mode, onApply, quiet = false }: ComposerAccessMenuProps) {
  const [open, setOpen] = useState(false);
  const label = `Agent access, ${runtimeModeLabel(mode)}`;

  return (
    <>
      {quiet ? (
        <Chip
          quiet
          accessibilityLabel={label}
          icon={MODE_ICONS[mode]}
          label={runtimeModeLabel(mode)}
          onPress={() => setOpen(true)}
        />
      ) : (
        // Icon only: the toolbar has no room for a labelled chip at phone
        // width. Full access tints so it stays noticeable.
        <ComposerIconButton
          active={mode === 'fullAccess'}
          icon={MODE_ICONS[mode]}
          label={label}
          onPress={() => setOpen(true)}
        />
      )}
      <AccessSheet
        mode={mode}
        onApply={onApply}
        onDismiss={() => setOpen(false)}
        visible={open}
      />
    </>
  );
}
