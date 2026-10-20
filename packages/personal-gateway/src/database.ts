import { Database, type SQLQueryBindings } from "bun:sqlite";
export type Binding = SQLQueryBindings;
export interface Statement<T, P extends Binding[]> {
  get(...values: P): T | null;
  all(...values: P): T[];
  run(...values: P): { changes: number; lastInsertRowid: number | bigint };
  finalize(): void;
}
/** A synchronous transaction boundary shared by local SQLite and Durable Objects.
 * Operations must not return a promise or perform I/O inside atomic(). */
export interface SqlDatabase {
  query<T = unknown, P extends Binding[] = Binding[]>(
    sql: string,
  ): Statement<T, P>;
  prepare<T = unknown, P extends Binding[] = Binding[]>(
    sql: string,
  ): Statement<T, P>;
  run(
    sql: string,
    values?: Binding[],
  ): { changes: number; lastInsertRowid: number | bigint };
  atomic<T>(operation: () => T): T;
  close(force?: boolean): void;
}
export type DatabaseSource = string | SqlDatabase;
export function openDatabase(source: DatabaseSource): SqlDatabase {
  if (typeof source !== "string") return source;
  const db = new Database(source, { create: true, strict: true });
  return {
    query<T, P extends Binding[]>(sql: string) {
      return db.query<T, P>(sql) as Statement<T, P>;
    },
    prepare<T, P extends Binding[]>(sql: string) {
      return db.prepare<T, P>(sql) as Statement<T, P>;
    },
    run(sql, values = []) {
      if (/^PRAGMA (?:busy_timeout|journal_mode)/i.test(sql)) {
        db.query(sql).all();
        return { changes: 0, lastInsertRowid: 0 };
      }
      return db.run(sql, values);
    },
    atomic(operation) {
      db.run("BEGIN IMMEDIATE");
      try {
        const value = operation();
        if (value instanceof Promise)
          throw new Error("SQL transactions must be synchronous");
        db.run("COMMIT");
        return value;
      } catch (error) {
        db.run("ROLLBACK");
        throw error;
      }
    },
    close: (force) => db.close(force),
  };
}
