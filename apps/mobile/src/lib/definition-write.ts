/** An existing draft cannot silently adopt a newer workspace revision. */
export function definitionWriteRevision(input: { editing: boolean; draftRevision: number | null; currentRevision: number; recordExists: boolean }): number {
  if (input.editing) {
    if (input.draftRevision === null || input.draftRevision !== input.currentRevision)
      throw new Error('This workspace changed. Reload from desktop before saving; your draft is kept until you reload.');
    return input.draftRevision;
  }
  if (input.recordExists) throw new Error('This draft already exists. Open it from Work to review the saved definition.');
  return input.currentRevision;
}
