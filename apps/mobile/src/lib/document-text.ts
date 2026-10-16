/** Original text is prepared only after immutable metadata/digest verification.
 * No browser TextDecoder/Buffer is required by the native runtime. */
export const MAX_DOCUMENT_TEXT_BYTES = 16 * 1024 * 1024;
export const TEXT_DOCUMENT_FORMATS = [
  "md",
  "txt",
  "csv",
  "json",
  "html",
  "htm",
  "svg",
  "xml",
  "yaml",
  "yml",
] as const;
export function isTextDocumentFormat(format: string): boolean {
  return (TEXT_DOCUMENT_FORMATS as readonly string[]).includes(format);
}

/** Bound uninterrupted preparation without paying a native timer for every
 * tiny piece. Callers check scope at each checkpoint, even before a yield. */
export function documentPreparation(
  current: () => boolean,
  label = "Document reading",
) {
  const clock = () => globalThis.performance?.now() ?? Date.now();
  let work = 0,
    started = clock();
  return async (units = 8192) => {
    if (!current()) throw new Error(`${label} scope changed`);
    work += units;
    if (work < 65536 && clock() - started < 4) return;
    await new Promise<void>((resolve) => setTimeout(resolve, 0));
    if (!current()) throw new Error(`${label} scope changed`);
    work = 0;
    started = clock();
  };
}

/** Yield on byte budgets, including multibyte sequences spanning a batch edge.
 * Each piece uses a bounded code-point array instead of one string per input
 * character retained until the complete original has been decoded. */
export function* documentTextSteps(bytes: Uint8Array): Generator<void, string> {
  if (bytes.length > MAX_DOCUMENT_TEXT_BYTES)
    throw new Error("Document text exceeds its 16 MiB limit");
  const pieces: string[] = [],
    points: number[] = [];
  let next = 8192;
  for (let i = 0; i < bytes.length;) {
    const first = bytes[i++]!;
    let point: number, count: number, min: number;
    if (first < 128) {
      point = first;
      count = 0;
      min = 0;
    } else if (first >= 194 && first <= 223) {
      point = first & 31;
      count = 1;
      min = 128;
    } else if (first >= 224 && first <= 239) {
      point = first & 15;
      count = 2;
      min = 2048;
    } else if (first >= 240 && first <= 244) {
      point = first & 7;
      count = 3;
      min = 65536;
    } else throw new Error("Document is not valid UTF-8 text");
    for (let j = 0; j < count; j++) {
      const following = bytes[i++];
      if (following === undefined || (following & 192) !== 128)
        throw new Error("Document is not valid UTF-8 text");
      point = (point << 6) | (following & 63);
    }
    if (point < min || point > 0x10ffff || (point >= 0xd800 && point <= 0xdfff))
      throw new Error("Document is not valid UTF-8 text");
    points.push(point);
    if (i >= next) {
      pieces.push(String.fromCodePoint(...points));
      points.length = 0;
      next = i + 8192;
      yield;
    }
  }
  if (points.length) pieces.push(String.fromCodePoint(...points));
  return pieces.join("");
}
export async function prepareDocumentText(
  bytes: Uint8Array,
  current: () => boolean,
): Promise<string> {
  const steps = documentTextSteps(bytes);
  const pause = documentPreparation(current);
  for (;;) {
    if (!current()) throw new Error("Document reading scope changed");
    const step = steps.next();
    if (step.done) {
      if (!current()) throw new Error("Document reading scope changed");
      return step.value;
    }
    await pause();
  }
}
