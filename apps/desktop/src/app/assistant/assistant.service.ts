import { inject, Injectable, signal } from "@angular/core";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import { SessionService } from "../session/session.service";
import { AssistantReply, ChatMessage } from "./assistant.model";

const ANSWER_WATCHDOG_MS = 240_000;
const MAX_CONTEXT_CHARS = 12_000;

function fitSignals(signals: readonly unknown[]): string {
  let kept = signals.length;
  while (
    kept > 0 &&
    JSON.stringify(signals.slice(0, kept)).length > MAX_CONTEXT_CHARS
  ) {
    kept -= 1;
  }
  if (kept === 0 && signals.length > 0) {
    return JSON.stringify(signals).slice(0, MAX_CONTEXT_CHARS + 1);
  }
  return JSON.stringify(signals.slice(0, kept));
}

@Injectable({ providedIn: "root" })
export class AssistantService {
  private readonly sessions = inject(SessionService);
  private readonly log = signal<readonly ChatMessage[]>([]);
  private readonly pending = signal(false);
  private watchdog: ReturnType<typeof setTimeout> | undefined;
  private turn = 0;

  readonly messages = this.log.asReadonly();
  readonly waiting = this.pending.asReadonly();

  constructor() {
    void listen<AssistantReply>("assistant", (event) => this.receive(event.payload));
  }

  async ask(text: string): Promise<void> {
    if (text === "" || this.pending()) {
      return;
    }

    this.log.update((all) => [
      ...all,
      { from: "player", text, pending: false, failed: false },
      { from: "warden", text: "...", pending: true, failed: false },
    ]);
    this.pending.set(true);
    const turn = ++this.turn;
    this.watchdog = setTimeout(
      () =>
        this.receive({
          turn,
          kind: "failed",
          message: "Le Gardien n'a pas répondu.",
        }),
      ANSWER_WATCHDOG_MS,
    );

    try {
      await invoke("assistant_ask", {
        turn,
        text,
        context: fitSignals(this.sessions.observation()?.signals ?? []),
        plugin: this.sessions.session()?.plugin ?? null,
      });
    } catch (error) {
      this.receive({ turn, kind: "failed", message: String(error) });
    }
  }

  async reset(): Promise<void> {
    if (this.pending()) {
      return;
    }
    try {
      await invoke("assistant_reset");
    } catch (error) {
      this.log.update((all) => [
        ...all,
        { from: "warden", text: String(error), pending: false, failed: true },
      ]);
      return;
    }
    this.log.set([]);
  }

  private receive(reply: AssistantReply): void {
    if (!this.pending() || reply.turn !== this.turn) {
      return;
    }
    this.log.update((all) => {
      const last = all.length - 1;
      if (last < 0 || all[last].from !== "warden") {
        return all;
      }

      const next: ChatMessage =
        reply.kind === "failed"
          ? { from: "warden", text: reply.message, pending: false, failed: true }
          : {
              from: "warden",
              text:
                reply.text === "" && reply.kind === "done"
                  ? "Le Gardien n'a rien à ajouter."
                  : reply.text,
              pending: reply.kind === "partial",
              failed: false,
            };
      return [...all.slice(0, last), next];
    });

    if (reply.kind !== "partial") {
      clearTimeout(this.watchdog);
      this.pending.set(false);
    }
  }
}
