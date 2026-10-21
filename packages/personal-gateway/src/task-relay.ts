import { openDatabase, type DatabaseSource, type SqlDatabase, type Binding as SQLQueryBindings } from "./database";
import { createHash, randomUUID } from "node:crypto";

export interface RelayTask {
  id: string;
  projectId: string;
  projectName: string;
  title: string;
  provider: string;
  status: string;
  runtimeId: string | null;
  updatedAt: number;
  publisherSeenAt?: number | null;
  computerTarget?: RelayComputerTarget | null;
}

export interface RelayComputerTarget {
  taskId: string;
  runtimeId: string;
  appName: string;
  windowTitle: string;
  phase: "awaitingApproval" | "running" | "failed";
  visible: boolean;
}

export interface RelayEvent {
  cursor: number;
  taskId: string;
  kind: "message" | "activity" | "artifact" | "diff" | "approval" | "status";
  text: string;
  at: number;
}

export interface RelayInteractionInput {
  taskId: string;
  runtimeId: string;
  requestId: string;
  kind: "permission" | "question";
  title: string;
  detail: string;
  options: Array<{ id: string; label: string; allow: boolean }>;
  questions: Array<{
    id: string;
    header: string;
    question: string;
    options: Array<{ label: string; description?: string }>;
    multiSelect: boolean;
  }>;
}

export interface RelayInteraction extends RelayInteractionInput {
  id: string;
  fingerprint: string;
  status: "pending" | "resolved";
  publishedAt: number;
}

export type RelayCommandKind = "stop" | "steer" | "permission" | "answer";
export interface RelayCommand {
  id: string;
  deviceId: string;
  publisherId: string;
  taskId: string;
  runtimeId: string;
  kind: RelayCommandKind;
  payload: Record<string, string>;
  status: "queued" | "uncertain" | "applied" | "rejected";
  createdAt: number;
  updatedAt: number;
}

const uuid = /^[0-9a-f-]{36}$/;
const kinds = new Set<RelayEvent["kind"]>([
  "message",
  "activity",
  "artifact",
  "diff",
  "approval",
  "status",
]);
const commandKinds = new Set<RelayCommandKind>([
  "stop",
  "steer",
  "permission",
  "answer",
]);

function validTask(value: RelayTask): void {
  if (
    !value ||
    !uuid.test(value.id) ||
    !uuid.test(value.projectId) ||
    (value.runtimeId !== null && !uuid.test(value.runtimeId)) ||
    !Number.isSafeInteger(value.updatedAt) ||
    value.updatedAt < 0 ||
    typeof value.projectName !== "string" ||
    value.projectName.length > 240 ||
    typeof value.title !== "string" ||
    value.title.length > 240 ||
    typeof value.provider !== "string" ||
    value.provider.length > 80 ||
    typeof value.status !== "string" ||
    value.status.length > 40
  ) {
    throw new Error("Invalid task summary");
  }
}

function validCommand(
  kind: RelayCommandKind,
  payload: Record<string, string>,
): void {
  if (
    !commandKinds.has(kind) ||
    !payload ||
    typeof payload !== "object" ||
    Array.isArray(payload)
  ) {
    throw new Error("Invalid task command");
  }
  if (kind === "stop" && Object.keys(payload).length !== 0)
    throw new Error("Stop has no arguments");
  if (
    kind === "steer" &&
    (typeof payload.prompt !== "string" ||
      !payload.prompt.trim() ||
      payload.prompt.length > 8_000 ||
      Object.keys(payload).length !== 1)
  ) {
    throw new Error("Steer needs a bounded prompt");
  }
  if (
    (kind === "permission" || kind === "answer") &&
    (!uuid.test(payload.interactionId ?? "") ||
      !/^[a-f0-9]{64}$/.test(payload.fingerprint ?? "") ||
      (kind === "permission" &&
        (typeof payload.optionId !== "string" ||
          Object.keys(payload).length !== 3)) ||
      (kind === "answer" &&
        (typeof payload.answers !== "string" ||
          payload.answers.length > 8_000 ||
          Object.keys(payload).length !== 3)))
  )
    throw new Error("Interaction command needs an exact snapshot");
}

/** Durable outbound desktop feed and single-claim phone command inbox. */
export class DurableTaskRelay {
  private readonly db: SqlDatabase;

  constructor(
    path: DatabaseSource,
    private readonly now: () => number = Date.now,
  ) {
    this.db = openDatabase(path);
    this.db.run("PRAGMA busy_timeout = 5000");
    this.db.run("PRAGMA synchronous = FULL");
    this.db.run(
      "CREATE TABLE IF NOT EXISTS relay_tasks (id TEXT PRIMARY KEY, body TEXT NOT NULL, updated_at INTEGER NOT NULL, published_at INTEGER NOT NULL, publisher_id TEXT NOT NULL DEFAULT '')",
    );
    if (
      !this.db
        .query<{ name: string }, []>("PRAGMA table_info(relay_tasks)")
        .all()
        .some((column) => column.name === "publisher_id")
    ) {
      this.db.run(
        "ALTER TABLE relay_tasks ADD COLUMN publisher_id TEXT NOT NULL DEFAULT ''",
      );
    }
    this.db.run(
      "CREATE TABLE IF NOT EXISTS relay_events (cursor INTEGER PRIMARY KEY AUTOINCREMENT, event_id TEXT UNIQUE NOT NULL, task_id TEXT NOT NULL, kind TEXT NOT NULL, text TEXT NOT NULL, at INTEGER NOT NULL)",
    );
    this.db.run(
      "CREATE TABLE IF NOT EXISTS relay_event_ids (event_id TEXT PRIMARY KEY, task_id TEXT NOT NULL, digest TEXT NOT NULL, cursor INTEGER NOT NULL)",
    );
    const knownEvents = this.db
      .query<
        {
          event_id: string;
          task_id: string;
          kind: string;
          text: string;
          at: number;
          cursor: number;
        },
        []
      >(
        "SELECT event_id, task_id, kind, text, at, cursor FROM relay_events WHERE NOT EXISTS (SELECT 1 FROM relay_event_ids WHERE relay_event_ids.event_id = relay_events.event_id)",
      )
      .all();
    for (const event of knownEvents) {
      const digest = createHash("sha256")
        .update(
          JSON.stringify([event.task_id, event.kind, event.text, event.at]),
        )
        .digest("hex");
      this.db.run(
        "INSERT OR IGNORE INTO relay_event_ids (event_id, task_id, digest, cursor) VALUES (?1, ?2, ?3, ?4)",
        [event.event_id, event.task_id, digest, event.cursor],
      );
    }
    this.db.run(
      "CREATE TABLE IF NOT EXISTS relay_commands (id TEXT PRIMARY KEY, device_id TEXT NOT NULL, idempotency_key TEXT NOT NULL, task_id TEXT NOT NULL, runtime_id TEXT NOT NULL, kind TEXT NOT NULL, payload TEXT NOT NULL, status TEXT NOT NULL, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL, UNIQUE(device_id, idempotency_key))",
    );
    if (
      !this.db
        .query<{ name: string }, []>("PRAGMA table_info(relay_commands)")
        .all()
        .some((column) => column.name === "publisher_id")
    ) {
      this.db.run(
        "ALTER TABLE relay_commands ADD COLUMN publisher_id TEXT NOT NULL DEFAULT ''",
      );
      this.db.run(
        "UPDATE relay_commands SET publisher_id=COALESCE((SELECT publisher_id FROM relay_tasks WHERE relay_tasks.id=relay_commands.task_id),'') WHERE publisher_id=''",
      );
      this.db.run(
        "UPDATE relay_commands SET status='rejected' WHERE publisher_id='' AND status='queued'",
      );
    }
    this.db.run(
      "CREATE TABLE IF NOT EXISTS relay_publisher_presence (device_id TEXT PRIMARY KEY, seen_at INTEGER NOT NULL)",
    );
    this.db.run("CREATE TABLE IF NOT EXISTS relay_task_snapshots (publisher_id TEXT PRIMARY KEY, body TEXT NOT NULL)");
    this.db.run(
      "CREATE TABLE IF NOT EXISTS relay_interactions (id TEXT PRIMARY KEY, publisher_id TEXT NOT NULL, task_id TEXT NOT NULL, runtime_id TEXT NOT NULL, request_id TEXT NOT NULL, body TEXT NOT NULL, fingerprint TEXT NOT NULL, status TEXT NOT NULL, published_at INTEGER NOT NULL, UNIQUE(task_id, runtime_id, request_id))",
    );
    this.db.run(
      "CREATE TABLE IF NOT EXISTS relay_computer_targets (task_id TEXT PRIMARY KEY, publisher_id TEXT NOT NULL, runtime_id TEXT NOT NULL, body TEXT NOT NULL)",
    );
  }

  close(): void {
    this.db.close(true);
  }

  heartbeat(publisherId: string): { publishedAt: number } {
    if (!uuid.test(publisherId)) throw new Error("Invalid task publisher");
    const publishedAt = this.now();
    this.db.run(
      "INSERT INTO relay_publisher_presence (device_id, seen_at) VALUES (?1, ?2) ON CONFLICT(device_id) DO UPDATE SET seen_at = excluded.seen_at WHERE seen_at<=?3 OR seen_at>?2",
      [publisherId, publishedAt, publishedAt - 30_000],
    );
    return { publishedAt };
  }

  private write<T>(operation: () => T): T {
    return this.db.atomic(operation);
  }

  publishTask(task: RelayTask, publisherId: string): { changed: boolean } {
    validTask(task);
    if (!uuid.test(publisherId)) throw new Error("Invalid task publisher");
    return this.write(() => {
      const body = JSON.stringify(task);
      const previous = this.db
        .query<{ body: string; publisher_id: string }, [string]>(
          "SELECT body, publisher_id FROM relay_tasks WHERE id = ?1",
        )
        .get(task.id);
      if (
        previous &&
        previous.publisher_id &&
        previous.publisher_id !== publisherId
      )
        throw new Error("Task belongs to another publisher");
      if (previous?.body === body && previous.publisher_id === publisherId)
        return { changed: false };
      this.db.run(
        "INSERT INTO relay_tasks (id, body, updated_at, published_at, publisher_id) VALUES (?1, ?2, ?3, ?4, ?5) ON CONFLICT(id) DO UPDATE SET body = excluded.body, updated_at = excluded.updated_at, published_at = excluded.published_at, publisher_id = excluded.publisher_id",
        [task.id, body, task.updatedAt, this.now(), publisherId],
      );
      if (task.status !== "waiting") {
        this.db.run(
          "UPDATE relay_interactions SET status = 'resolved' WHERE task_id = ?1 AND status = 'pending'",
          [task.id],
        );
      } else {
        this.db.run(
          "UPDATE relay_interactions SET status = 'resolved' WHERE task_id = ?1 AND runtime_id <> ?2 AND status = 'pending'",
          [task.id, task.runtimeId],
        );
      }
      if (
        !task.runtimeId ||
        task.status === "idle" ||
        task.status === "failed"
      ) {
        this.db.run("DELETE FROM relay_computer_targets WHERE task_id = ?1", [
          task.id,
        ]);
      } else {
        this.db.run(
          "DELETE FROM relay_computer_targets WHERE task_id = ?1 AND runtime_id <> ?2",
          [task.id, task.runtimeId],
        );
      }
      this.db.run(
        "INSERT INTO relay_events (event_id, task_id, kind, text, at) VALUES (?1, ?2, ?3, ?4, ?5)",
        [randomUUID(), task.id, "status", task.status, this.now()],
      );
      this.pruneEvents();
      return { changed: true };
    });
  }

  reconcileTasks(
    publisherId: string,
    activeIds: string[],
  ): { removed: number } {
    if (
      !uuid.test(publisherId) ||
      !Array.isArray(activeIds) ||
      activeIds.length > 200 ||
      activeIds.some((id) => !uuid.test(id)) ||
      new Set(activeIds).size !== activeIds.length
    ) {
      throw new Error("Invalid task snapshot");
    }
    return this.write(() => {
      const owned = this.db
        .query<{ id: string }, [string]>(
          "SELECT id FROM relay_tasks WHERE publisher_id = ?1",
        )
        .all(publisherId);
      const active = new Set(activeIds);
      let removed = 0;
      for (const { id } of owned) {
        if (active.has(id)) continue;
        this.db.run("DELETE FROM relay_tasks WHERE id = ?1", [id]);
        this.db.run("DELETE FROM relay_events WHERE task_id = ?1", [id]);
        this.db.run("DELETE FROM relay_event_ids WHERE task_id = ?1", [id]);
        this.db.run("DELETE FROM relay_computer_targets WHERE task_id = ?1", [
          id,
        ]);
        this.db.run(
          "UPDATE relay_interactions SET status = 'resolved' WHERE task_id = ?1",
          [id],
        );
        this.db.run(
          "UPDATE relay_commands SET status = 'rejected', updated_at = ?2 WHERE task_id = ?1 AND status = 'queued'",
          [id, this.now()],
        );
        removed++;
      }
      // A small receipt lets a restarted desktop distinguish a restored relay
      // from a merely unchanged task list without scanning every task each poll.
      this.db.run("INSERT INTO relay_task_snapshots VALUES (?1,?2) ON CONFLICT(publisher_id) DO UPDATE SET body=excluded.body WHERE body<>excluded.body",
        [publisherId, JSON.stringify([...activeIds].sort())]);
      return { removed };
    });
  }
  hasTaskSnapshot(publisherId: string): boolean {
    return !!this.db.query<{ present: number }, [string]>("SELECT 1 AS present FROM relay_task_snapshots WHERE publisher_id=?1").get(publisherId);
  }

  setComputerTarget(
    publisherId: string,
    target: RelayComputerTarget,
  ): { visible: boolean } {
    if (
      !uuid.test(publisherId) ||
      !target ||
      !uuid.test(target.taskId) ||
      !uuid.test(target.runtimeId) ||
      typeof target.appName !== "string" ||
      target.appName.length > 240 ||
      typeof target.windowTitle !== "string" ||
      target.windowTitle.length > 500 ||
      !["awaitingApproval", "running", "failed"].includes(target.phase) ||
      typeof target.visible !== "boolean"
    )
      throw new Error("Invalid computer target");
    return this.write(() => {
      const task = this.db
        .query<{ body: string; publisher_id: string }, [string]>(
          "SELECT body, publisher_id FROM relay_tasks WHERE id = ?1",
        )
        .get(target.taskId);
      if (
        !task ||
        task.publisher_id !== publisherId ||
        (JSON.parse(task.body) as RelayTask).runtimeId !== target.runtimeId
      ) {
        throw new Error("Task runtime changed or is unavailable");
      }
      if (target.visible) {
        this.db.run(
          "INSERT INTO relay_computer_targets (task_id, publisher_id, runtime_id, body) VALUES (?1, ?2, ?3, ?4) ON CONFLICT(task_id) DO UPDATE SET runtime_id = excluded.runtime_id, body = excluded.body",
          [
            target.taskId,
            publisherId,
            target.runtimeId,
            JSON.stringify(target),
          ],
        );
      } else {
        this.db.run("DELETE FROM relay_computer_targets WHERE task_id = ?1", [
          target.taskId,
        ]);
      }
      return { visible: target.visible };
    });
  }

  publishInteraction(
    publisherId: string,
    input: RelayInteractionInput,
  ): RelayInteraction & { created: boolean } {
    if (
      !uuid.test(publisherId) ||
      !input ||
      !uuid.test(input.taskId) ||
      !uuid.test(input.runtimeId) ||
      Object.keys(input).sort().join(",") !==
        "detail,kind,options,questions,requestId,runtimeId,taskId,title" ||
      typeof input.requestId !== "string" ||
      !input.requestId ||
      input.requestId.length > 200 ||
      !["permission", "question"].includes(input.kind) ||
      typeof input.title !== "string" ||
      input.title.length > 500 ||
      typeof input.detail !== "string" ||
      input.detail.length > 10_000 ||
      !Array.isArray(input.options) ||
      input.options.length > 20 ||
      !Array.isArray(input.questions) ||
      input.questions.length > 5 ||
      (input.kind === "permission" &&
        (input.options.length === 0 || input.questions.length !== 0)) ||
      (input.kind === "question" &&
        (input.questions.length === 0 || input.options.length !== 0)) ||
      input.options.some(
        (option) =>
          !option ||
          typeof option !== "object" ||
          Object.keys(option).sort().join(",") !== "allow,id,label" ||
          typeof option.id !== "string" ||
          !option.id ||
          option.id.length > 200 ||
          typeof option.label !== "string" ||
          option.label.length > 500 ||
          typeof option.allow !== "boolean",
      ) ||
      input.questions.some(
        (question) =>
          !question ||
          typeof question !== "object" ||
          Object.keys(question).sort().join(",") !==
            "header,id,multiSelect,options,question" ||
          typeof question.id !== "string" ||
          !question.id ||
          question.id.length > 200 ||
          typeof question.header !== "string" ||
          question.header.length > 300 ||
          typeof question.question !== "string" ||
          question.question.length > 2_000 ||
          typeof question.multiSelect !== "boolean" ||
          !Array.isArray(question.options) ||
          question.options.length > 20 ||
          question.options.some(
            (option) =>
              !option ||
              typeof option !== "object" ||
              !["label", "description,label"].includes(
                Object.keys(option).sort().join(","),
              ) ||
              typeof option.label !== "string" ||
              option.label.length > 500 ||
              (option.description !== undefined &&
                (typeof option.description !== "string" ||
                  option.description.length > 1_000)),
          ),
      )
    ) {
      throw new Error("Invalid provider interaction");
    }
    return this.write(() => {
      const task = this.db
        .query<{ body: string; publisher_id: string }, [string]>(
          "SELECT body, publisher_id FROM relay_tasks WHERE id = ?1",
        )
        .get(input.taskId);
      if (
        !task ||
        task.publisher_id !== publisherId ||
        (JSON.parse(task.body) as RelayTask).runtimeId !== input.runtimeId ||
        (JSON.parse(task.body) as RelayTask).status !== "waiting"
      ) {
        throw new Error("Task runtime changed or is unavailable");
      }
      const body = JSON.stringify(input);
      const fingerprint = createHash("sha256").update(body).digest("hex");
      const previous = this.db
        .query<{ id: string; fingerprint: string }, [string, string, string]>(
          "SELECT id, fingerprint FROM relay_interactions WHERE task_id = ?1 AND runtime_id = ?2 AND request_id = ?3",
        )
        .get(input.taskId, input.runtimeId, input.requestId);
      if (previous) {
        if (previous.fingerprint !== fingerprint)
          throw new Error(
            "Provider interaction changed under the same request ID",
          );
        return { ...this.interaction(previous.id)!, created: false };
      }
      const id = randomUUID();
      const publishedAt = this.now();
      this.db.run(
        "INSERT INTO relay_interactions (id, publisher_id, task_id, runtime_id, request_id, body, fingerprint, status, published_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        [
          id,
          publisherId,
          input.taskId,
          input.runtimeId,
          input.requestId,
          body,
          fingerprint,
          "pending",
          publishedAt,
        ],
      );
      return {
        ...input,
        id,
        fingerprint,
        status: "pending",
        publishedAt,
        created: true,
      };
    });
  }

  interaction(id: string): RelayInteraction | null {
    const row = this.db
      .query<
        {
          body: string;
          id: string;
          fingerprint: string;
          status: "pending" | "resolved";
          published_at: number;
        },
        [string]
      >(
        "SELECT id, body, fingerprint, status, published_at FROM relay_interactions WHERE id = ?1",
      )
      .get(id);
    return row
      ? {
          ...(JSON.parse(row.body) as RelayInteractionInput),
          id: row.id,
          fingerprint: row.fingerprint,
          status: row.status,
          publishedAt: row.published_at,
        }
      : null;
  }

  interactionForPublisher(
    publisherId: string,
    id: string,
  ): RelayInteraction | null {
    const row = this.db
      .query<{ publisher_id: string }, [string]>(
        "SELECT publisher_id FROM relay_interactions WHERE id = ?1",
      )
      .get(id);
    return row?.publisher_id === publisherId ? this.interaction(id) : null;
  }

  pendingInteractions(taskId: string): RelayInteraction[] {
    if (!uuid.test(taskId)) throw new Error("Invalid task ID");
    const rows = this.db
      .query<{ id: string }, [string]>(
        "SELECT id FROM relay_interactions WHERE task_id = ?1 AND status = 'pending' ORDER BY published_at LIMIT 20",
      )
      .all(taskId);
    return rows.map((row) => this.interaction(row.id)!);
  }

  resolveInteraction(publisherId: string, id: string): RelayInteraction {
    return this.write(() => {
      const row = this.db
        .query<{ publisher_id: string }, [string]>(
          "SELECT publisher_id FROM relay_interactions WHERE id = ?1",
        )
        .get(id);
      if (!row || row.publisher_id !== publisherId)
        throw new Error("Interaction not found");
      this.db.run(
        "UPDATE relay_interactions SET status = 'resolved' WHERE id = ?1",
        [id],
      );
      this.db.run(
        "UPDATE relay_commands SET status = 'rejected', updated_at = ?2 WHERE status = 'queued' AND (kind = 'permission' OR kind = 'answer') AND json_extract(payload, '$.interactionId') = ?1",
        [id, this.now()],
      );
      return this.interaction(id)!;
    });
  }

  resolveInteractionRequest(
    publisherId: string,
    taskId: string,
    runtimeId: string,
    requestId: string,
  ): RelayInteraction | null {
    if (
      !uuid.test(publisherId) ||
      !uuid.test(taskId) ||
      !uuid.test(runtimeId) ||
      typeof requestId !== "string" ||
      !requestId ||
      requestId.length > 200
    ) {
      throw new Error("Invalid interaction identity");
    }
    const row = this.db
      .query<{ id: string }, [string, string, string, string]>(
        "SELECT id FROM relay_interactions WHERE publisher_id = ?1 AND task_id = ?2 AND runtime_id = ?3 AND request_id = ?4",
      )
      .get(publisherId, taskId, runtimeId, requestId);
    return row ? this.resolveInteraction(publisherId, row.id) : null;
  }

  private validateInteractionCommand(
    taskId: string,
    runtimeId: string,
    kind: RelayCommandKind,
    payload: Record<string, string>,
  ): void {
    if (kind !== "permission" && kind !== "answer") return;
    const interaction = this.interaction(payload.interactionId!);
    if (
      !interaction ||
      interaction.status !== "pending" ||
      interaction.taskId !== taskId ||
      interaction.runtimeId !== runtimeId ||
      interaction.fingerprint !== payload.fingerprint ||
      this.now() - interaction.publishedAt > 3_600_000
    ) {
      throw new Error("Provider interaction is no longer pending");
    }
    if (kind === "permission") {
      if (
        interaction.kind !== "permission" ||
        !interaction.options.some((option) => option.id === payload.optionId)
      ) {
        throw new Error("Permission option does not match the exact review");
      }
    } else {
      if (interaction.kind !== "question")
        throw new Error("Answer does not match the pending question");
      let answers: unknown;
      try {
        answers = JSON.parse(payload.answers!);
      } catch {
        throw new Error("Invalid question answers");
      }
      if (
        !Array.isArray(answers) ||
        answers.length !== interaction.questions.length ||
        answers.some(
          (entry) =>
            !entry ||
            typeof entry !== "object" ||
            typeof entry.questionId !== "string" ||
            !Array.isArray(entry.answers) ||
            entry.answers.length === 0 ||
            entry.answers.length > 5 ||
            entry.answers.some(
              (answer: unknown) =>
                typeof answer !== "string" ||
                !answer.trim() ||
                answer.length > 2_000,
            ),
        ) ||
        interaction.questions.some((question) => {
          const answer = answers.find(
            (entry) => entry.questionId === question.id,
          );
          return (
            !answer || (!question.multiSelect && answer.answers.length !== 1)
          );
        }) ||
        new Set(answers.map((entry) => entry.questionId)).size !==
          answers.length
      ) {
        throw new Error("Invalid question answers");
      }
    }
  }

  taskForPublisher(publisherId: string, taskId: string): RelayTask | null {
    const strictUuid =
      /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;
    if (!strictUuid.test(publisherId) || !strictUuid.test(taskId))
      throw new Error("Invalid task owner identity");
    const row = this.db
      .query<{ body: string; seen_at: number | null }, [string, string]>(
        "SELECT t.body,p.seen_at FROM relay_tasks t LEFT JOIN relay_publisher_presence p ON p.device_id=t.publisher_id WHERE t.id=?1 AND t.publisher_id=?2",
      )
      .get(taskId, publisherId);
    if (!row) return null;
    const task = JSON.parse(row.body) as RelayTask;
    return {
      id: task.id,
      projectId: task.projectId,
      projectName: task.projectName,
      title: task.title,
      provider: task.provider,
      status: task.status,
      runtimeId: task.runtimeId,
      updatedAt: task.updatedAt,
      publisherSeenAt: row.seen_at,
    };
  }

  listTasks(): { tasks: RelayTask[]; publishedAt: number | null } {
    const rows = this.db
      .query<
        { body: string; seen_at: number | null; computer_body: string | null },
        []
      >(
        "SELECT relay_tasks.body, relay_publisher_presence.seen_at, relay_computer_targets.body AS computer_body FROM relay_tasks LEFT JOIN relay_publisher_presence ON relay_publisher_presence.device_id = relay_tasks.publisher_id LEFT JOIN relay_computer_targets ON relay_computer_targets.task_id = relay_tasks.id ORDER BY relay_tasks.updated_at DESC LIMIT 200",
      )
      .all();
    return {
      tasks: rows.map((row) => ({
        ...(JSON.parse(row.body) as RelayTask),
        publisherSeenAt: row.seen_at,
        computerTarget: row.computer_body
          ? (JSON.parse(row.computer_body) as RelayComputerTarget)
          : null,
      })),
      publishedAt: rows.reduce<number | null>(
        (latest, row) => Math.max(latest ?? 0, row.seen_at ?? 0) || null,
        null,
      ),
    };
  }

  appendEvent(
    eventId: string,
    taskId: string,
    kind: RelayEvent["kind"],
    text: string,
    at: number,
  ): RelayEvent {
    if (
      !eventId ||
      eventId.length > 160 ||
      !uuid.test(taskId) ||
      !kinds.has(kind) ||
      typeof text !== "string" ||
      text.length > 48_000 ||
      !Number.isSafeInteger(at) ||
      at < 0
    )
      throw new Error("Invalid task event");
    return this.write(() => {
      const digest = createHash("sha256")
        .update(JSON.stringify([taskId, kind, text, at]))
        .digest("hex");
      const previous = this.db
        .query<{ cursor: number; digest: string }, [string]>(
          "SELECT cursor, digest FROM relay_event_ids WHERE event_id = ?1",
        )
        .get(eventId);
      if (previous) {
        if (previous.digest !== digest) {
          throw new Error("Event ID was reused with different content");
        }
        return { cursor: previous.cursor, taskId, kind, text, at };
      }
      if (!this.db.query("SELECT 1 FROM relay_tasks WHERE id = ?1").get(taskId))
        throw new Error("Task is not published");
      this.db.run(
        "INSERT INTO relay_events (event_id, task_id, kind, text, at) VALUES (?1, ?2, ?3, ?4, ?5)",
        [eventId, taskId, kind, text, at],
      );
      const row = this.db
        .query<{ cursor: number }, []>("SELECT last_insert_rowid() AS cursor")
        .get()!;
      this.db.run(
        "INSERT INTO relay_event_ids (event_id, task_id, digest, cursor) VALUES (?1, ?2, ?3, ?4)",
        [eventId, taskId, digest, row.cursor],
      );
      this.pruneEvents();
      return { cursor: row.cursor, taskId, kind, text, at };
    });
  }

  events(
    after: number,
    limit = 100,
  ): { events: RelayEvent[]; nextCursor: number; resetRequired: boolean } {
    if (
      !Number.isSafeInteger(after) ||
      after < 0 ||
      !Number.isInteger(limit) ||
      limit < 1 ||
      limit > 200
    ) {
      throw new Error("Invalid event cursor");
    }
    const oldest =
      this.db
        .query<{ cursor: number }, []>(
          "SELECT MIN(cursor) AS cursor FROM relay_events",
        )
        .get()?.cursor ?? 0;
    const resetRequired = oldest > 0 && after > 0 && after < oldest - 1;
    const rows = this.db
      .query<RelayEvent, [number, number]>(
        "SELECT cursor, task_id AS taskId, kind, text, at FROM relay_events WHERE cursor > ?1 ORDER BY cursor LIMIT ?2",
      )
      .all(resetRequired ? oldest - 1 : after, limit);
    return {
      events: rows,
      nextCursor: rows.at(-1)?.cursor ?? after,
      resetRequired,
    };
  }

  private pruneEvents(): void {
    this.db.run(
      "DELETE FROM relay_events WHERE cursor <= (SELECT COALESCE(MAX(cursor) - 10000, 0) FROM relay_events)",
    );
  }

  enqueue(
    deviceId: string,
    idempotencyKey: string,
    taskId: string,
    runtimeId: string,
    kind: RelayCommandKind,
    payload: Record<string, string>,
    expectedPublisherId?: string,
  ): RelayCommand {
    if (
      !uuid.test(deviceId) ||
      !uuid.test(idempotencyKey) ||
      !uuid.test(taskId) ||
      !uuid.test(runtimeId)
    ) {
      throw new Error("Invalid task command identity");
    }
    validCommand(kind, payload);
    if (expectedPublisherId !== undefined && !uuid.test(expectedPublisherId))
      throw new Error("Invalid task command owner");
    return this.write(() => {
      const existing = this.db
        .query<{ id: string }, [string, string]>(
          "SELECT id FROM relay_commands WHERE device_id = ?1 AND idempotency_key = ?2",
        )
        .get(deviceId, idempotencyKey);
      if (existing) {
        const command = this.command(existing.id)!;
        if (
          (expectedPublisherId !== undefined &&
            command.publisherId !== expectedPublisherId) ||
          command.taskId !== taskId ||
          command.runtimeId !== runtimeId ||
          command.kind !== kind ||
          JSON.stringify(command.payload) !== JSON.stringify(payload)
        ) {
          throw new Error("Idempotency key was reused for another command");
        }
        return command;
      }
      const task = this.db
        .query<{ body: string; publisher_id: string }, [string]>(
          "SELECT body,publisher_id FROM relay_tasks WHERE id = ?1",
        )
        .get(taskId);
      if (
        !task ||
        (expectedPublisherId !== undefined &&
          task.publisher_id !== expectedPublisherId) ||
        (JSON.parse(task.body) as RelayTask).runtimeId !== runtimeId
      ) {
        throw new Error("Task runtime changed or is unavailable");
      }
      this.validateInteractionCommand(taskId, runtimeId, kind, payload);
      const id = randomUUID();
      const time = this.now();
      this.db.run(
        "INSERT INTO relay_commands (id, device_id, idempotency_key, task_id, runtime_id, kind, payload, status, created_at, updated_at, publisher_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        [
          id,
          deviceId,
          idempotencyKey,
          taskId,
          runtimeId,
          kind,
          JSON.stringify(payload),
          "queued",
          time,
          time,
          task.publisher_id,
        ],
      );
      return this.command(id)!;
    });
  }

  command(id: string): RelayCommand | null {
    const row = this.db
      .query<
        {
          id: string;
          device_id: string;
          publisher_id: string;
          task_id: string;
          runtime_id: string;
          kind: RelayCommandKind;
          payload: string;
          status: RelayCommand["status"];
          created_at: number;
          updated_at: number;
        },
        [string]
      >("SELECT * FROM relay_commands WHERE id = ?1")
      .get(id);
    if (!row) return null;
    return {
      id: row.id,
      deviceId: row.device_id,
      publisherId: row.publisher_id,
      taskId: row.task_id,
      runtimeId: row.runtime_id,
      kind: row.kind,
      payload: JSON.parse(row.payload) as Record<string, string>,
      status: row.status,
      createdAt: row.created_at,
      updatedAt: row.updated_at,
    };
  }

  pendingCommands(publisherId: string): RelayCommand[] {
    const rows = this.db
      .query<{ id: string }, [string]>(
        "SELECT id FROM relay_commands WHERE status='queued' AND publisher_id=?1 ORDER BY CASE WHEN kind='stop' THEN 0 ELSE 1 END,created_at LIMIT 50",
      )
      .all(publisherId);
    return rows.map((row) => this.command(row.id)!);
  }

  claim(id: string, publisherId: string): RelayCommand {
    return this.write(() => {
      const command = this.command(id);
      if (!command || !publisherId || command.publisherId !== publisherId)
        throw new Error("Command belongs to another publisher");
      if (command?.status === "rejected") return command;
      if (!command || command.status !== "queued")
        throw new Error("Command has no unused claim");
      const task = this.db
        .query<{ body: string; publisher_id: string }, [string]>(
          "SELECT body,publisher_id FROM relay_tasks WHERE id = ?1",
        )
        .get(command.taskId);
      if (
        !task ||
        task.publisher_id !== publisherId ||
        (JSON.parse(task.body) as RelayTask).runtimeId !== command.runtimeId
      ) {
        this.db.run(
          "UPDATE relay_commands SET status = 'rejected', updated_at = ?2 WHERE id = ?1",
          [id, this.now()],
        );
        return this.command(id)!;
      }
      try {
        this.validateInteractionCommand(
          command.taskId,
          command.runtimeId,
          command.kind,
          command.payload,
        );
      } catch {
        this.db.run(
          "UPDATE relay_commands SET status = 'rejected', updated_at = ?2 WHERE id = ?1",
          [id, this.now()],
        );
        return this.command(id)!;
      }
      this.db.run(
        "UPDATE relay_commands SET status = 'uncertain', updated_at = ?2 WHERE id = ?1",
        [id, this.now()],
      );
      return this.command(id)!;
    });
  }

  settle(id: string, applied: boolean, publisherId: string): RelayCommand {
    return this.write(() => {
      const command = this.command(id);
      if (!command || !publisherId || command.publisherId !== publisherId)
        throw new Error("Command belongs to another publisher");
      if (!command || command.status !== "uncertain")
        throw new Error("Command is not awaiting a result");
      this.db.run(
        "UPDATE relay_commands SET status = ?2, updated_at = ?3 WHERE id = ?1",
        [id, applied ? "applied" : "rejected", this.now()],
      );
      if (
        applied &&
        (command.kind === "permission" || command.kind === "answer")
      ) {
        this.db.run(
          "UPDATE relay_interactions SET status = 'resolved' WHERE id = ?1",
          [command.payload.interactionId!],
        );
      }
      if (applied && command.kind === "stop") {
        this.db.run(
          "UPDATE relay_commands SET status = 'rejected', updated_at = ?3 WHERE task_id = ?1 AND runtime_id = ?2 AND status = 'queued'",
          [command.taskId, command.runtimeId, this.now()],
        );
      }
      return this.command(id)!;
    });
  }
}
