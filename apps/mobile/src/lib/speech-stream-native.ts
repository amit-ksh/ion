import { Platform } from 'react-native';
import {
  CloudSpeechStream,
  type SpeechSocket,
  type SpeechSocketFactory,
} from './speech-stream';

// React Native supports request headers as its third WebSocket argument. Browsers
// do not; never put the private service token in a URL or subprotocol workaround.
type NativeWebSocket = new (
  url: string,
  protocols: string[],
  options: { headers: Record<string, string> },
) => SpeechSocket;
export const nativeSpeechSocket: SpeechSocketFactory = (url, headers) => {
  if (Platform.OS === 'web')
    throw new Error('Microphone streaming requires the native phone app');
  return new (WebSocket as unknown as NativeWebSocket)(url, [], { headers });
};
export { CloudSpeechStream };
