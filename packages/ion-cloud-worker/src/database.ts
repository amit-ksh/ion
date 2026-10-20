import type {
  Binding,
  SqlDatabase,
  Statement,
} from "../../personal-gateway/src/database";
/** Consume every cursor before returning. A cursor must never span an await. */
export function durableDatabase(storage: DurableObjectStorage): SqlDatabase {
  function binding(value: Binding): string | number | null | ArrayBuffer {
    if (typeof value === "boolean") return Number(value);
    if (typeof value === "bigint") {
      const n = Number(value);
      if (!Number.isSafeInteger(n)) throw new Error("Unsafe SQL integer");
      return n;
    }
    if (
      value === null ||
      typeof value === "string" ||
      typeof value === "number"
    )
      return value;
    if (ArrayBuffer.isView(value))
      return Uint8Array.from(
        new Uint8Array(value.buffer, value.byteOffset, value.byteLength),
      ).buffer;
    throw new Error("Unsupported SQL binding");
  }
  function rows<T>(sql: string, values: Binding[]): T[] {
    // File locking/durability are provided by the Durable Object, not PRAGMAs.
    if (
      /^PRAGMA\s+(?:busy_timeout|journal_mode|synchronous)\s*=/i.test(
        sql.trim(),
      )
    )
      return [];
    if (/^(?:BEGIN|COMMIT|ROLLBACK)\b/i.test(sql.trim()))
      throw new Error("Use atomic() for Durable Object transactions");
    return storage.sql.exec(sql, ...values.map(binding)).toArray() as T[];
  }
  function run(sql: string, values: Binding[] = []) {
    rows(sql, values);
    return rows<{ changes: number; lastInsertRowid: number }>(
      "SELECT changes() AS changes,last_insert_rowid() AS lastInsertRowid",
      [],
    )[0]!;
  }
  function statement<T, P extends Binding[]>(sql: string): Statement<T, P> {
    return {
      get: (...values) => rows<T>(sql, values)[0] ?? null,
      all: (...values) => rows<T>(sql, values),
      run: (...values) => run(sql, values),
      finalize() {},
    };
  }
  return {
    query: statement,
    prepare: statement,
    run,
    atomic(operation) {
      return storage.transactionSync(() => {
        const value = operation();
        if (value instanceof Promise)
          throw new Error("SQL transactions must be synchronous");
        return value;
      });
    },
    close() {},
  };
}
