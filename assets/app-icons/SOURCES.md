# App identity assets

These bundled marks identify existing integrations. They do not add integrations,
imply endorsement or change app permissions. Brand names and marks belong to
their respective owners. The JSON manifest records source URLs and byte hashes.

- Gmail, Google Drive, Google Calendar, Google Sheets and Google Docs: unmodified
  product PNGs from Google's static asset hosts, matching the style in the
  supplied reference.
- Outlook: unmodified product SVG from Microsoft's Office CDN.
- GitHub, Supabase and ElevenLabs: Simple Icons artwork from its public CDN,
  distributed under [CC0-1.0](https://github.com/simple-icons/simple-icons/blob/develop/LICENSE.md).
  The GitHub and ElevenLabs marks use theme ink for contrast; Supabase retains
  its supplied color.
- GitLab: the existing repository file-type asset is reused without changing it.

Desktop embeds all asset bytes. Mobile embeds the PNGs as data URIs and the SVG XML;
the SVG title metadata is omitted from mobile XML because each row already has
an accessible app name. Existing GitLab inline fill styles are normalized to SVG
attributes.

Apps without a bundled mark (Slack, Notion, Linear and the rest of the catalog)
use the public CompanyEnrich logo endpoint (`api.companyenrich.com/logo/<domain>`),
keyless and cached on disk for 30 days. Only the company domain leaves the
device. Until a logo arrives, or when the request fails, the generic app glyph
shows.
