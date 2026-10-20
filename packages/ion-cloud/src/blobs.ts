import { mkdir, readFile, writeFile, lstat, rm } from "node:fs/promises";
import { join } from "node:path";
import { createHash } from "node:crypto";
export interface BlobStore {
  put(workspace: string, digest: string, content: Uint8Array): Promise<void>;
  get(workspace: string, digest: string): Promise<Buffer<ArrayBuffer>>;
  delete(workspace: string, digest: string): Promise<void>;
}
export function objectKey(workspace: string, digest: string) {
  if (!/^[a-f0-9]{64}$/.test(workspace) || !/^[a-f0-9]{64}$/.test(digest))
    throw new Error("Invalid object identity");
  return workspace + "/" + digest;
}
export function verifyObject(digest: string, content: Uint8Array) {
  if (createHash("sha256").update(content).digest("hex") !== digest)
    throw new Error("Invalid stored object");
}
export class LocalBlobs implements BlobStore {
  constructor(private readonly directory: string) {}
  private path(workspace: string, digest: string) {
    objectKey(workspace, digest);
    return join(this.directory, "objects", workspace, digest);
  }
  async put(workspace: string, digest: string, content: Uint8Array) {
    verifyObject(digest, content);
    const target = this.path(workspace, digest),
      parent = join(this.directory, "objects", workspace);
    await mkdir(parent, { recursive: true, mode: 0o700 });
    for (const directory of [join(this.directory, "objects"), parent]) {
      const info = await lstat(directory);
      if (!info.isDirectory() || info.isSymbolicLink())
        throw new Error("Invalid object directory");
    }
    try {
      await writeFile(target, content, { flag: "wx", mode: 0o600 });
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code !== "EEXIST") throw error;
      verifyObject(digest, await this.get(workspace, digest));
    }
  }
  async get(workspace: string, digest: string) {
    const path = this.path(workspace, digest),
      info = await lstat(path);
    if (!info.isFile() || info.isSymbolicLink() || info.size > 20_000_000)
      throw new Error("Invalid stored object");
    const bytes = await readFile(path);
    verifyObject(digest, bytes);
    if (!(bytes.buffer instanceof ArrayBuffer))
      throw new Error("Unsupported object buffer");
    return bytes as Buffer<ArrayBuffer>;
  }
  async delete(workspace: string, digest: string) {
    await rm(this.path(workspace, digest), { force: true });
  }
}
