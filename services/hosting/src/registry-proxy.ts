/** CONNECT-only dependency egress. No arbitrary URLs or host IPs are accepted. */
import { createServer } from "node:http";
import { connect } from "node:net";
import { lookup } from "node:dns/promises";
const publicV4 = (ip: string) => {
  const p = ip.split(".").map(Number);
  return (
    p.length === 4 &&
    p.every((v) => Number.isInteger(v) && v >= 0 && v <= 255) &&
    ![0, 10, 127, 169, 172, 192, 224, 240, 255].includes(p[0]!) &&
    p[0]! < 224 &&
    !(p[0] === 100 && p[1]! >= 64 && p[1]! <= 127) &&
    !(p[0] === 198 && (p[1] === 18 || p[1] === 19))
  );
};
export function startRegistryProxy(host: string, port: number) {
  if (
    !/^172\.3[01]\.\d{1,3}\.1$/.test(host) ||
    !Number.isInteger(port) ||
    port < 1024 ||
    port > 65535
  )
    throw new Error(
      "Registry proxy must bind the dedicated internal bridge gateway",
    );
  const allowed = new Set([
    "registry.npmjs.org",
    "pypi.org",
    "files.pythonhosted.org",
  ]);
  const server = createServer((_, response) => {
    response.writeHead(403);
    response.end();
  });
  let connections = 0;
  server.on("connect", (request, client, head) => {
    const match = /^([a-z0-9.-]+):443$/.exec(request.url ?? "");
    if (!match || !allowed.has(match[1]!) || connections >= 64 || head.length) {
      client.destroy();
      return;
    }
    connections++;
    let released = false;
    const release = () => {
      if (!released) {
        released = true;
        connections--;
      }
    };
    client.once("close", release);
    client.once("error", () => client.destroy());
    void (async () => {
      const address = await lookup(match[1]!, { family: 4 });
      if (!publicV4(address.address))
        throw new Error("Registry address rejected");
      const upstream = connect({ host: address.address, port: 443 });
      upstream.setTimeout(60_000, () => upstream.destroy());
      const clientTimer = setTimeout(() => client.destroy(), 60_000);
      client.once("close", () => clearTimeout(clientTimer));
      let bytes = 0;
      upstream.on("data", (chunk) => {
        bytes += chunk.length;
        if (bytes > 100_000_000) {
          upstream.destroy();
          client.destroy();
        }
      });
      upstream.once("connect", () => {
        client.write("HTTP/1.1 200 Connection Established\r\n\r\n");
        client.pipe(upstream);
        upstream.pipe(client);
      });
      upstream.once("error", () => client.destroy());
      client.once("close", () => upstream.destroy());
    })().catch(() => client.destroy());
  });
  server.listen(port, host);
  return server;
}
