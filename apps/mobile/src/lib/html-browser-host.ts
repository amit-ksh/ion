import type { HtmlHostAdapter } from "./html-resource-host";
export {
  allowResourceNavigation,
  openHtmlResourceHost,
} from "./html-resource-host";
async function activeWorker() {
  if (!("serviceWorker" in navigator) || !globalThis.isSecureContext)
    throw new Error("Linked documents require a secure browser context");
  const url = new URL("/ion-document/worker.js", location.origin);
  const registration = await navigator.serviceWorker.register(url, {
    scope: "/ion-document/",
  });
  if (registration.scope !== new URL("/ion-document/", location.origin).href)
    throw new Error("Document worker scope changed");
  const worker =
    registration.installing ?? registration.waiting ?? registration.active;
  if (!worker || worker.scriptURL !== url.href)
    throw new Error("Document worker unavailable");
  if (worker.state !== "activated")
    await new Promise<void>((resolve, reject) => {
      const finish = () => {
        if (worker.state === "activated" || worker.state === "redundant") {
          clearTimeout(timer);
          worker.removeEventListener("statechange", finish);
          worker.state === "activated"
            ? resolve()
            : reject(new Error("Document worker stopped"));
        }
      };
      const timer = setTimeout(() => {
        worker.removeEventListener("statechange", finish);
        reject(new Error("Document worker timed out"));
      }, 10_000);
      worker.addEventListener("statechange", finish);
      finish();
    });
  return worker;
}
export async function browserHtmlHost(): Promise<HtmlHostAdapter> {
  const worker = await activeWorker();
  const send = <T>(input: object) =>
    new Promise<T>((resolve, reject) => {
      const channel = new MessageChannel();
      const timer = setTimeout(() => {
        channel.port1.close();
        reject(new Error("Document host timed out"));
      }, 5000);
      channel.port1.onmessage = (event) => {
        clearTimeout(timer);
        channel.port1.close();
        event.data?.ok
          ? resolve(event.data.value)
          : reject(new Error("Document host refused"));
      };
      worker.postMessage(input, [channel.port2]);
    });
  return {
    open: (token, entry, files) => send({ type: "open", token, entry, files }),
    close: (token) => send({ type: "close", token }),
    renew: (token) => send({ type: "renew", token }),
  };
}
