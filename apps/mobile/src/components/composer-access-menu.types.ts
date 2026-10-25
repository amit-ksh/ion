import type { RuntimeMode } from '@waku/client';

export interface ComposerAccessMenuProps {
  mode: RuntimeMode;
  onApply: (mode: RuntimeMode) => void;
  /** Render as a labelled context chip (under the composer) instead of a toolbar icon. */
  quiet?: boolean;
}
