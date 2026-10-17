import type { HtmlResourceDocument } from "./html-resource-document";
export interface HtmlHostAdapter {
  open(
    token: string,
    entry: string,
    files: HtmlResourceDocument["files"],
  ): Promise<{ token: string; url: string }>;
  close(token: string): Promise<void>;
  renew(token: string): Promise<boolean>;
}
export function allowResourceNavigation(url: string, entry: string) {
  return (
    url === entry ||
    (url.startsWith(entry + "#") && !/[\x00-\x20\x7f]/.test(url))
  );
}
/** Delayed native work cannot make a superseded document visible or keep its
 * host alive. Callers renew only while owner/route/grant/foreground remain valid. */
export async function openHtmlResourceHost(
  document: HtmlResourceDocument,
  token: string,
  current: () => boolean,
  adapter: HtmlHostAdapter,
  webOrigin?: string,
) {
  if (!current()) throw new Error("Document host scope changed");
  let result: Awaited<ReturnType<HtmlHostAdapter["open"]>>;
  try {
    result = await adapter.open(token, document.entryPath, document.files);
    const url = new URL(result.url);
    const origin = webOrigin
      ? url.origin === webOrigin
      : url.protocol === "http:" && url.hostname === "127.0.0.1" && !!url.port;
    const path =
      (webOrigin ? "/ion-document/" : "/") + token + "/" + document.entryPath;
    if (
      !current() ||
      result.token !== token ||
      !origin ||
      url.username ||
      url.password ||
      url.search ||
      url.hash ||
      decodeURIComponent(url.pathname) !== path
    )
      throw new Error("Invalid or superseded document host");
  } catch (cause) {
    await adapter.close(token).catch(() => {});
    throw cause;
  }
  let closed = false;
  const close = async () => {
    if (closed) return;
    closed = true;
    await adapter.close(token);
  };
  return {
    url: result.url,
    close,
    async renew() {
      if (closed) return false;
      if (!current()) {
        await close();
        return false;
      }
      try {
        const alive = await adapter.renew(token);
        if (!alive || !current()) {
          await close();
          return false;
        }
        return true;
      } catch (cause) {
        await close().catch(() => {});
        throw cause;
      }
    },
  };
}
