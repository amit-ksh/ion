import { MenuView, type MenuAction } from '@expo/ui/community/menu';
import { StyleSheet, View } from 'react-native';

import { AppSymbol } from './app-symbol';
import { useTheme } from '@/hooks/use-theme';

export type OverflowAction = { id: string; title: string; image?: MenuAction['image']; destructive?: boolean };

/**
 * A 48dp "more" trigger that opens the platform menu. MenuView owns the
 * gesture, so the trigger is a labelled, non-pressable View (see
 * HeaderMenuTrigger) and nested pressables are avoided on Android.
 */
export function OverflowMenu({ label, actions, onAction, disabled = false }: {
  label: string;
  actions: OverflowAction[];
  onAction: (id: string) => void;
  disabled?: boolean;
}) {
  const theme = useTheme();
  const trigger = (
    <View accessible accessibilityLabel={label} accessibilityRole="button" accessibilityState={{ disabled }}
      style={[styles.trigger, { opacity: disabled ? 0.45 : 1 }]}>
      <AppSymbol name={{ ios: 'ellipsis', android: 'more_horiz', web: 'more_horiz' }} size={20} tintColor={theme.textSecondary} />
    </View>
  );
  if (disabled || !actions.length) return trigger;
  const menu: MenuAction[] = actions.map(action => ({
    id: action.id,
    title: action.title,
    ...(action.image ? { image: action.image } : {}),
    ...(action.destructive ? { attributes: { destructive: true } } : {}),
  }));
  return (
    <MenuView actions={menu} onPressAction={({ nativeEvent }) => onAction(nativeEvent.event)}>
      {trigger}
    </MenuView>
  );
}

const styles = StyleSheet.create({
  trigger: { alignItems: 'center', height: 48, justifyContent: 'center', width: 44 },
});
