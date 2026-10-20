import { CloudFailure, type Account } from "./auth";
import { Budget, money, tokenCost, type Limits } from "./budget";
import { readJson } from "./service-store";
export interface PricedModel {
  id: string; contextTokens: number; outputTokens: number;
  inputMicroUSDPerMillion: number; outputMicroUSDPerMillion: number;
}
export interface InferencePolicy { models: PricedModel[]; limits: Limits }
export function inferencePolicy(raw?: string): InferencePolicy | undefined {
  if (!raw) return;
  const p = JSON.parse(raw) as InferencePolicy;
  if (!p || !Array.isArray(p.models) || !p.models.length || p.models.length > 8 ||
      !p.limits || p.limits.lifetime > 25_000_000 || p.limits.monthly > 10_000_000 ||
      new Set(p.models.map(m => m.id)).size !== p.models.length) throw new Error("Invalid inference policy");
  for (const key of ["monthly", "lifetime", "accountLifetime"] as const) {
    const n = p.limits[key]; money(n); if (!n) throw new Error("Missing spending limit");
  }
  for (const m of p.models) {
    if (!/^[A-Za-z0-9_./-]{1,200}$/.test(m.id) || !Number.isSafeInteger(m.contextTokens) ||
        m.contextTokens < 1024 || m.contextTokens > 1_000_000 || !Number.isSafeInteger(m.outputTokens) ||
        m.outputTokens < 1 || m.outputTokens > 32768 || m.outputTokens >= m.contextTokens)
      throw new Error("Invalid approved model limits");
    money(m.inputMicroUSDPerMillion); money(m.outputMicroUSDPerMillion);
    if (!m.inputMicroUSDPerMillion || !m.outputMicroUSDPerMillion) throw new Error("Set verified conservative model prices");
  }
  return p;
}
/** Bounded account-scoped inference with real SSE forwarding. No prompt,
 * response, API key or access token is stored in the spending ledger. */
export class Inference {
  readonly available: boolean;
  private inFlight = new Set<string>();
  constructor(private budget: Budget, private enabled: () => boolean,
    private key?: string, private policy?: InferencePolicy,
    private send: (url: string, init: RequestInit) => Promise<Response> = (url, init) => globalThis.fetch(url, init)) { this.available = !!key && !!policy; }
  async handle(account: Account, request: Request): Promise<Response> {
    if (!this.available) throw new CloudFailure(503, "Shared inference is not configured");
    if (!this.enabled()) throw new CloudFailure(403, "Shared Token Factory is OFF");
    const path = new URL(request.url).pathname;
    if (path === "/v1/inference/models" && request.method === "GET")
      return Response.json({ object: "list", data: this.policy!.models.map(m => ({ id: m.id, object: "model", context_length: m.contextTokens })) }, { headers: { "Cache-Control": "no-store" } });
    if (path !== "/v1/inference/chat/completions" || request.method !== "POST")
      throw new CloudFailure(405, "Unsupported inference operation");
    if (this.inFlight.size >= 4) throw new CloudFailure(429, "Shared inference concurrency limit reached");
    if (this.inFlight.has(account.workspaceId)) throw new CloudFailure(429, "An inference request is already active for this account");
    this.inFlight.add(account.workspaceId);
    let streaming = false;
    try {
      const response = await this.complete(account, request);
      streaming = response.headers.get("content-type")?.includes("text/event-stream") ?? false;
      return response;
    } finally { if (!streaming) this.inFlight.delete(account.workspaceId); }
  }
  private async complete(account: Account, request: Request) {
    const body = await readJson(request, 500_000), model = this.policy!.models.find(m => m.id === body?.model);
    if (!model || !Array.isArray(body.messages) || !body.messages.length || body.messages.length > 128 ||
        body.messages.some((m: any) => !m || !["system", "user", "assistant", "tool"].includes(m.role) ||
          (m.content !== null && typeof m.content !== "string")) ||
        (body.n !== undefined && body.n !== 1) || (body.stream !== undefined && typeof body.stream !== "boolean") ||
        (body.tools !== undefined && (!Array.isArray(body.tools) || body.tools.length > 64 ||
          body.tools.some((t: any) => !t || t.type !== "function" || !t.function ||
            !/^[A-Za-z0-9_-]{1,64}$/.test(t.function.name) ||
            (t.function.description !== undefined && (typeof t.function.description !== "string" || t.function.description.length > 8192))))))
      throw new CloudFailure(400, "Use an approved model and bounded text/tool conversation");
    const output = body.max_completion_tokens ?? body.max_tokens ?? model.outputTokens;
    if (!Number.isSafeInteger(output) || output < 1 || output > model.outputTokens)
      throw new CloudFailure(400, "Output token limit exceeds the approved maximum");
    // Text-only pilot: a conservative UTF-8 byte + template envelope prevents
    // a misconfigured context policy from accepting an arbitrarily larger prompt.
    const envelope = 2 * new TextEncoder().encode(JSON.stringify({ messages: body.messages, tools: body.tools })).length
      + 1024 * (body.messages.length + (body.tools?.length ?? 0) + 1);
    if (envelope + output > model.contextTokens)
      throw new CloudFailure(413, "Conversation exceeds the approved conservative context envelope");
    // Reserve a full context, not a tokenizer estimate. A provider-accepted input
    // cannot exceed the configured model context; prices must be operator upper bounds.
    const maximum = tokenCost(model.contextTokens, model.inputMicroUSDPerMillion) + tokenCost(output, model.outputMicroUSDPerMillion);
    const reservation = this.budget.reserve("tokenFactory", account.workspaceId, maximum, this.policy!.limits);
    const input = { model: model.id, messages: body.messages, max_completion_tokens: output, n: 1, stream: body.stream === true,
      ...(body.stream === true ? { stream_options: { include_usage: true } } : {}),
      ...(body.tools?.length ? { tools: body.tools, tool_choice: body.tool_choice ?? "auto" } : {}) };
    let response: Response;
    try {
      response = await this.send("https://api.tokenfactory.nebius.com/v1/chat/completions", {
        method: "POST", headers: { Authorization: "Bearer " + this.key, "Content-Type": "application/json" },
        body: JSON.stringify(input), redirect: "manual", signal: AbortSignal.timeout(90_000),
      });
    } catch { throw new CloudFailure(503, "Inference outcome is uncertain; its spending reservation is retained"); }
    if (!response.ok) {
      await response.body?.cancel();
      // Provider rejection may still consume billable work; do not assume a refund.
      throw new CloudFailure(502, "Token Factory rejected the request; reservation retained");
    }
    if (body.stream === true) return this.stream(response, model, output, reservation, () => this.inFlight.delete(account.workspaceId));
    const result = await readJson(new Request("https://result.invalid", { method: "POST", body: response.body, duplex: "half" } as RequestInit), 2_000_000);
    const usage = result?.usage, prompt = usage?.prompt_tokens, completion = usage?.completion_tokens;
    if (result.model !== model.id || !Number.isSafeInteger(prompt) || prompt < 0 || prompt > model.contextTokens ||
        !Number.isSafeInteger(completion) || completion < 0 || completion > output ||
        usage.total_tokens !== prompt + completion || !Array.isArray(result.choices) || result.choices.length !== 1)
      throw new CloudFailure(502, "Incomplete inference receipt; reservation retained");
    this.budget.settle(reservation, tokenCost(prompt, model.inputMicroUSDPerMillion) + tokenCost(completion, model.outputMicroUSDPerMillion));
    const headers = { "Cache-Control": "no-store", "X-Content-Type-Options": "nosniff" };
    return Response.json(result, { headers });
  }
  private stream(response: Response, model: PricedModel, output: number, reservation: string, release: () => void): Response {
    if (!response.body || !response.headers.get("content-type")?.includes("text/event-stream"))
      throw new CloudFailure(502, "Missing inference stream; reservation retained");
    const reader = response.body.getReader(), encoder = new TextEncoder(), decoder = new TextDecoder("utf-8", { fatal: true, ignoreBOM: false });
    let canceled = false;
    const budget = this.budget;
    const body = new ReadableStream<Uint8Array>({
      async start(controller) {
        let bytes = 0, buffered = "", receipt: { prompt: number; completion: number } | undefined, finished = false, done = false;
        try {
          while (!done) {
            const chunk = await reader.read();
            if (chunk.done) break;
            bytes += chunk.value.byteLength;
            if (bytes > 2_000_000) throw new Error("stream limit");
            buffered += decoder.decode(chunk.value, { stream: true });
            while (buffered.includes("\n")) {
              const end = buffered.indexOf("\n"), line = buffered.slice(0, end).trimEnd();
              buffered = buffered.slice(end + 1);
              if (line.length > 131072) throw new Error("event limit");
              if (!line.startsWith("data:")) continue;
              const data = line.slice(5).trimStart();
              if (data === "[DONE]") { done = true; break; }
              const packet = JSON.parse(data);
              if (packet.error || (packet.model !== undefined && packet.model !== model.id) ||
                  (packet.choices !== undefined && (!Array.isArray(packet.choices) || packet.choices.length > 1 || packet.choices.some((c: any) => c.index !== 0))))
                throw new Error("invalid provider event");
              if (packet.choices?.[0]?.finish_reason) finished = true;
              if (packet.usage != null) {
                const u = packet.usage;
                if (!Number.isSafeInteger(u.prompt_tokens) || u.prompt_tokens < 0 || u.prompt_tokens > model.contextTokens ||
                    !Number.isSafeInteger(u.completion_tokens) || u.completion_tokens < 0 || u.completion_tokens > output ||
                    u.total_tokens !== u.prompt_tokens + u.completion_tokens ||
                    (receipt && (receipt.prompt !== u.prompt_tokens || receipt.completion !== u.completion_tokens)))
                  throw new Error("invalid usage");
                receipt = { prompt: u.prompt_tokens, completion: u.completion_tokens };
              }
              // Keep provider event order; expose only completion fields, never an
              // upstream auth envelope, internal tracing object or provider error.
              const safe = { id: packet.id, object: packet.object, model: packet.model, choices: packet.choices ?? [],
                ...(packet.usage != null ? { usage: packet.usage } : {}) };
              controller.enqueue(encoder.encode("data: " + JSON.stringify(safe) + "\n\n"));
            }
            if (buffered.length > 131072) throw new Error("event limit");
          }
          if (!done || !finished || !receipt) throw new Error("incomplete stream");
          budget.settle(reservation, tokenCost(receipt.prompt, model.inputMicroUSDPerMillion) + tokenCost(receipt.completion, model.outputMicroUSDPerMillion));
          if (!canceled) { controller.enqueue(encoder.encode("data: [DONE]\n\n")); controller.close(); }
        } catch {
          if (!canceled) {
            controller.enqueue(encoder.encode('data: {"error":{"message":"Inference stream incomplete; spending remains reserved unless a complete receipt was settled."}}\n\n'));
            controller.close();
          }
        } finally { release(); await reader.cancel().catch(() => {}); }
      },
      async cancel() { canceled = true; release(); await reader.cancel().catch(() => {}); },
    });
    return new Response(body, { headers: { "Content-Type": "text/event-stream", "Cache-Control": "no-store", "X-Content-Type-Options": "nosniff" } });
  }
}
