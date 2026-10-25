import { expect, test } from 'bun:test';

import { appLogoUrl } from './app-logo';

test('app logos use the company domain, defaulting to the slug', () => {
  expect(appLogoUrl('slack')).toBe('https://api.companyenrich.com/logo/slack.com');
  expect(appLogoUrl('notion')).toBe('https://api.companyenrich.com/logo/notion.so');
  expect(appLogoUrl('jira')).toBe('https://api.companyenrich.com/logo/atlassian.com');
  expect(appLogoUrl('one_drive')).toBe('https://api.companyenrich.com/logo/microsoft.com');
  expect(appLogoUrl('zoom')).toBe('https://api.companyenrich.com/logo/zoom.us');
});

test('only plain toolkit slugs are requested', () => {
  expect(appLogoUrl('')).toBeNull();
  expect(appLogoUrl('Slack')).toBeNull();
  expect(appLogoUrl('../admin')).toBeNull();
  expect(appLogoUrl('a'.repeat(65))).toBeNull();
});
