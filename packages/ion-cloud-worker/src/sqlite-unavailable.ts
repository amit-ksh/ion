/** The Workers bundle must inject Durable Object SQL; never open a local file. */
export class Database {
  constructor() {
    throw new Error("A Durable Object SQL binding is required");
  }
}
