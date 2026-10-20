import {
  objectKey,
  verifyObject,
  type BlobStore,
} from "../../ion-cloud/src/blobs";
/** R2 is private: only the gateway can read, write or delete these objects. */
export class R2Blobs implements BlobStore {
  constructor(private readonly bucket: R2Bucket) {}
  async put(workspace: string, digest: string, content: Uint8Array) {
    const key = objectKey(workspace, digest);
    verifyObject(digest, content);
    await this.bucket.put(key, content, { customMetadata: { sha256: digest } });
  }
  async get(workspace: string, digest: string) {
    const object = await this.bucket.get(objectKey(workspace, digest));
    if (!object || object.size > 20_000_000)
      throw new Error("Object unavailable");
    const bytes = Buffer.from(await object.arrayBuffer());
    verifyObject(digest, bytes);
    return bytes;
  }
  async delete(workspace: string, digest: string) {
    await this.bucket.delete(objectKey(workspace, digest));
  }
}
