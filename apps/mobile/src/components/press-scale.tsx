import { useRef, useState } from 'react';
import {
  Animated,
  Easing,
  Pressable,
  type PressableProps,
  type StyleProp,
  type ViewStyle,
} from 'react-native';

import { useReducedMotion } from '@/hooks/use-reduced-motion';

const AnimatedPressable = Animated.createAnimatedComponent(Pressable);
const easeOut = Easing.bezier(0, 0, 0.58, 1);

export type PressableScaleProps = Omit<PressableProps, 'style'> & {
  style?: StyleProp<ViewStyle> | ((state: { pressed: boolean }) => StyleProp<ViewStyle>);
};

/**
 * Pressable with tactile feedback: scales to 0.96 over 150ms ease-out and
 * back. Disabled controls never move, and under reduced motion only the
 * caller's pressed styling (fill or opacity) remains as the static cue.
 */
export function PressableScale({ style, disabled, onPressIn, onPressOut, ...props }: PressableScaleProps) {
  const reduced = useReducedMotion();
  const scale = useRef(new Animated.Value(1)).current;
  const [pressed, setPressed] = useState(false);
  const animate = (toValue: number) =>
    Animated.timing(scale, { toValue, duration: 150, easing: easeOut, useNativeDriver: true }).start();
  return (
    <AnimatedPressable
      {...props}
      disabled={disabled}
      onPressIn={(event) => {
        setPressed(true);
        if (!disabled && !reduced) animate(0.96);
        onPressIn?.(event);
      }}
      onPressOut={(event) => {
        setPressed(false);
        animate(1);
        onPressOut?.(event);
      }}
      style={[typeof style === 'function' ? style({ pressed }) : style, { transform: [{ scale }] }]}
    />
  );
}
