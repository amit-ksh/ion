// Company domains for toolkits whose slug is not `{slug}.com`, matching desktop.
const DOMAINS: Record<string, string> = {
  bitbucket: 'bitbucket.org',
  confluence: 'atlassian.com',
  excel: 'microsoft.com',
  jira: 'atlassian.com',
  linear: 'linear.app',
  microsoftteams: 'microsoft.com',
  notion: 'notion.so',
  one_drive: 'microsoft.com',
  onedrive: 'microsoft.com',
  perplexityai: 'perplexity.ai',
  sentry: 'sentry.io',
  sharepoint: 'microsoft.com',
  slackbot: 'slack.com',
  telegram: 'telegram.org',
  twitter: 'x.com',
  zoom: 'zoom.us',
};

/** CompanyEnrich's free, keyless logo endpoint for apps without a bundled
 * mark. Unknown domains get a neutral initials tile. Only plain toolkit slugs
 * become a URL path segment. */
export function appLogoUrl(toolkit: string): string | null {
  if (!/^[a-z0-9_-]{1,64}$/.test(toolkit)) return null;
  return `https://api.companyenrich.com/logo/${DOMAINS[toolkit] ?? `${toolkit}.com`}`;
}
