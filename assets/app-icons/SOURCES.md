# App identity assets

These bundled marks identify existing integrations. They do not add integrations,
imply endorsement or change app permissions. Brand names and marks belong to
their respective owners. The JSON manifest records source URLs and byte hashes.

- Gmail and Google Drive: unmodified classic product PNGs from Google's static
  asset hosts, matching the style in the supplied reference.
- Outlook: unmodified product SVG from Microsoft's Office CDN.
- GitHub and Supabase: Simple Icons artwork from its public CDN, distributed
  under [CC0-1.0](https://github.com/simple-icons/simple-icons/blob/develop/LICENSE.md).
  The GitHub mark uses theme ink for contrast; Supabase retains its supplied color.
- GitLab: the existing repository file-type asset is reused without changing it.

Desktop embeds all asset bytes. Mobile bundles the two PNGs and embeds SVG XML;
the SVG title metadata is omitted from mobile XML because each row already has
an accessible app name. Existing GitLab inline fill styles are normalized to SVG
attributes. No runtime download, remote SVG reference or new package is required.
