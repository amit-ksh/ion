import { expect, test } from 'bun:test';

import { appDisplayName } from './app-display-name';

test('readable names replace raw toolkit slugs only', () => {
  expect(appDisplayName('googlecalendar', 'googlecalendar')).toBe('Google Calendar');
  expect(appDisplayName('elevenlabs', 'elevenlabs')).toBe('ElevenLabs');
  expect(appDisplayName('trello', 'trello')).toBe('Trello');
  expect(appDisplayName('github', 'Work GitHub')).toBe('Work GitHub');
});
