import { inject, Injectable, signal } from "@angular/core";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import { SessionService } from "../session/session.service";
import { AssistantReply, ChatMessage } from "./assistant.model";

const ANSWER_WATCHDOG_MS = 200_000;

@Injectable({ providedIn: "root" })
export class AssistantService {
  private readonly sessions = inject(SessionService);
  private readonly log = signal<readonly ChatMessage[]>([]);
  private readonly pending = signal(false);
  private watchdog: ReturnType<typeof setTimeout> | undefined;

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
    this.watchdog = setTimeout(
      () =>
        this.receive({
          kind: "failed",
          message: "Le Gardien n'a pas répondu.",
        }),
      ANSWER_WATCHDOG_MS,
    );

    try {
      await invoke("assistant_ask", {
        text,
        context: JSON.stringify(this.sessions.observation()?.signals ?? []),
        plugin: this.sessions.session()?.plugin ?? null,
      });
    } catch (error) {
      this.receive({ kind: "failed", message: String(error) });
    }
  }

  async reset(): Promise<void> {
    if (this.pending()) {
      return;
    }
    try {
      await invoke("assistant_reset");
    } catch {
      return;
    }
    this.log.set([]);
  }

  private receive(reply: AssistantReply): void {
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
              text: reply.text,
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
