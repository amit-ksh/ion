const NAMES: Record<string, string> = {
  elevenlabs: 'ElevenLabs',
  github: 'GitHub',
  gitlab: 'GitLab',
  gmail: 'Gmail',
  googlecalendar: 'Google Calendar',
  googledocs: 'Google Docs',
  googledrive: 'Google Drive',
  googlesheets: 'Google Sheets',
  linear: 'Linear',
  notion: 'Notion',
  outlook: 'Outlook',
  slack: 'Slack',
  supabase: 'Supabase',
};

/** Connected-account lists can carry the raw toolkit slug as the name.
 * Show a readable one then, matching desktop; a real name is kept as is. */
export function appDisplayName(slug: string, name: string): string {
  if (name !== slug) return name;
  return NAMES[slug] ?? slug.charAt(0).toUpperCase() + slug.slice(1);
}
