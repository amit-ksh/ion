import { requireNativeView, requireOptionalNativeModule } from 'expo';
import { Platform, useColorScheme, type ViewProps } from 'react-native';

const supported = Platform.OS === 'android' && requireOptionalNativeModule<{ sheetSystemBarsSupported?: boolean }>('IonDocumentHost')?.sheetSystemBarsSupported;
const NativeSystemBars = supported ? requireNativeView<ViewProps & { dark: boolean }>('IonDocumentHost') : null;

/** Mounted inside the native sheet so its view belongs to the dialog window. */
export function SheetSystemBars() {
  const scheme = useColorScheme();
  return NativeSystemBars ? <NativeSystemBars dark={scheme === 'dark'} pointerEvents="none" accessible={false} style={{ width: 0, height: 0 }} /> : null;
}
