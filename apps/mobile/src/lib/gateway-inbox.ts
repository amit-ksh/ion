import {
  inboxSubmission,
  sameInboxSubmission,
  validateInboxSubmission,
  validateInboxCommand,
  type InboxCommand,
  type InboxSubmission,
} from "@waku/client";
import type { SubmissionStorage } from "./automation-api";
import {
  gatewayEnqueueInbox,
  gatewayInboxByKey,
  GatewayRequestError,
  type GatewayPairing,
} from "./gateway-client";
interface Transport {
  lookup(pairing: GatewayPairing, key: string): Promise<InboxCommand | null>;
  enqueue(
    pairing: GatewayPairing,
    input: InboxSubmission,
  ): Promise<InboxCommand>;
}
const actual: Transport = {
  lookup: gatewayInboxByKey,
  enqueue: gatewayEnqueueInbox,
};
const active = new Map<string, Promise<InboxCommand>>();
export type InboxIntent = Pick<
  InboxSubmission,
  "sequence" | "expectedRevision" | "action"
>;
/** Save the exact expiring operation before network I/O. A lost acknowledgement
 * retains its nonce and cannot become a fresh action on a later notification. */
export async function submitGatewayInbox(
  pairing: GatewayPairing,
  storage: SubmissionStorage,
  publisherId: string,
  intent: InboxIntent,
  nonce: () => string,
  current: () => boolean,
  transport: Transport = actual,
): Promise<InboxCommand> {
  const allowed = () =>
    current() &&
    pairing.expiresAt > Date.now() &&
    pairing.grants.includes("inboxRead") &&
    pairing.grants.includes("inboxControl");
  if (!allowed())
    throw new Error(
      "Pair with Inbox read and control grants before changing notifications.",
    );
  const key = `ion.mobile.gateway-inbox.v1.${encodeURIComponent(pairing.baseUrl)}.${pairing.id}.${publisherId}.${intent.sequence}`;
  const previous = active.get(key);
  if (previous) return previous;
  const operation = perform();
  active.set(key, operation);
  try {
    return await operation;
  } finally {
    if (active.get(key) === operation) active.delete(key);
  }
  async function perform(): Promise<InboxCommand> {
    const saved = await storage.getItem(key);
    let input: InboxSubmission;
    if (saved !== null) {
      try {
        input = JSON.parse(saved);
        validateInboxSubmission(input);
      } catch {
        throw new Error(
          "Saved Inbox request is invalid. Keep it for recovery.",
        );
      }
      if (
        input.publisherId !== publisherId ||
        input.sequence !== intent.sequence
      )
        throw new Error("Saved Inbox request belongs to another notification.");
    } else {
      input = {
        publisherId,
        ...intent,
        idempotencyKey: nonce(),
        expiresAt: Date.now() + 120_000,
      };
      validateInboxSubmission(input);
      await storage.setItem(key, JSON.stringify(input));
    }
    const check = () => {
      if (!allowed())
        throw new Error(
          "Inbox scope changed. Keep the saved request for recovery.",
        );
    };
    const verify = (command: InboxCommand) => {
      validateInboxCommand(command);
      if (
        command.deviceId !== pairing.id ||
        !sameInboxSubmission(input, inboxSubmission(command))
      )
        throw new Error(
          "Inbox command identity changed. Keep the saved request for recovery.",
        );
    };
    check();
    let command =
      saved === null
        ? null
        : await transport.lookup(pairing, input.idempotencyKey);
    check();
    if (command) verify(command);
    if (!command) {
      if (input.expiresAt <= Date.now()) {
        // Confirmed absence plus the pinned expired deadline proves this operation
        // cannot still mutate the owner; the next tap may create a fresh request.
        await storage.removeItem(key);
        throw new Error(
          "Earlier Inbox request expired. Refresh and try again.",
        );
      }
      if (
        input.action !== intent.action ||
        input.expectedRevision !== intent.expectedRevision
      )
        throw new Error(
          "Earlier Inbox request differs. Recover it before sending another action.",
        );
      check();
      try {
        command = await transport.enqueue(pairing, input);
        verify(command);
      } catch (error) {
        check();
        const recovered = await transport
          .lookup(pairing, input.idempotencyKey)
          .catch(() => undefined);
        check();
        if (recovered) {
          verify(recovered);
          command = recovered;
        } else {
          if (
            recovered === null &&
            error instanceof GatewayRequestError &&
            [400, 409, 422].includes(error.status)
          )
            await storage.removeItem(key);
          throw error;
        }
      }
    }
    check();
    if (command.status === "applied" || command.status === "rejected")
      await storage.removeItem(key);
    return command;
  }
}
