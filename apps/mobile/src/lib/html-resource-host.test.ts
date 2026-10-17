import { expect, test } from "bun:test";
import {
  openHtmlResourceHost,
  allowResourceNavigation,
  type HtmlHostAdapter,
} from "./html-resource-host";
const token = "2737752b-0d1f-4f65-9192-ae30fbb5eab8";
const doc = { entryPath: "経緯/plot.html", files: [] };
function fixture(
  url = "http://127.0.0.1:45871/" + token + "/%E7%B5%8C%E7%B7%AF/plot.html",
) {
  const closed: string[] = [];
  const adapter: HtmlHostAdapter = {
    open: async () => ({ token, url }),
    close: async (id) => {
      closed.push(id);
    },
    renew: async () => true,
  };
  return { adapter, closed };
}
test("a verified lease permits only its document and fragments; close is final", async () => {
  const { adapter, closed } = fixture();
  const lease = await openHtmlResourceHost(doc, token, () => true, adapter);
  expect(await lease.renew()).toBe(true);
  expect(allowResourceNavigation(lease.url, lease.url)).toBe(true);
  expect(allowResourceNavigation(lease.url + "#figure", lease.url)).toBe(true);
  for (const value of [
    lease.url + "?x",
    lease.url + "/other",
    "https://outside.invalid",
    "file:///private",
    "about:blank",
    lease.url + "#a\n",
  ])
    expect(allowResourceNavigation(value, lease.url)).toBe(false);
  await lease.close();
  await lease.close();
  expect(await lease.renew()).toBe(false);
  expect(closed).toEqual([token]);
});
test("delayed open for a superseded owner is torn down before exposure", async () => {
  const { adapter, closed } = fixture();
  let current = true;
  const open = adapter.open;
  adapter.open = async (...args) => {
    const value = await open(...args);
    current = false;
    return value;
  };
  await expect(
    openHtmlResourceHost(doc, token, () => current, adapter),
  ).rejects.toThrow("superseded");
  expect(closed).toEqual([token]);
});
test("host response cannot redirect authority, path, token or attach credentials", async () => {
  for (const url of [
    "https://outside.invalid/doc",
    "http://0.0.0.0:80/" + token + "/経緯/plot.html",
    "http://127.0.0.1:45871/wrong/経緯/plot.html",
    "http://owner:secret@127.0.0.1:45871/" + token + "/経緯/plot.html",
    "http://127.0.0.1:45871/" + token + "/経緯/plot.html?q=x",
  ]) {
    const { adapter, closed } = fixture(url);
    await expect(
      openHtmlResourceHost(doc, token, () => true, adapter),
    ).rejects.toThrow();
    expect(closed).toEqual([token]);
  }
});
test("renewal cannot outlive revocation or a failed native lease", async () => {
  for (const failure of ["scope", "expired", "error"]) {
    const { adapter, closed } = fixture();
    let current = true;
    const lease = await openHtmlResourceHost(
      doc,
      token,
      () => current,
      adapter,
    );
    if (failure === "scope") current = false;
    if (failure === "expired") adapter.renew = async () => false;
    if (failure === "error")
      adapter.renew = async () => {
        throw Error("native stopped");
      };
    if (failure === "error") await expect(lease.renew()).rejects.toThrow();
    else expect(await lease.renew()).toBe(false);
    expect(await lease.renew()).toBe(false);
    expect(closed).toEqual([token]);
  }
});
test("browser lease remains within the dedicated worker scope", async () => {
  const { adapter } = fixture(
    "https://ion.test/ion-document/" + token + "/経緯/plot.html",
  );
  const lease = await openHtmlResourceHost(
    doc,
    token,
    () => true,
    adapter,
    "https://ion.test",
  );
  expect(await lease.renew()).toBe(true);
  await lease.close();
  const { adapter: wrong } = fixture(
    "https://ion.test/account/" + token + "/経緯/plot.html",
  );
  await expect(
    openHtmlResourceHost(doc, token, () => true, wrong, "https://ion.test"),
  ).rejects.toThrow();
});
