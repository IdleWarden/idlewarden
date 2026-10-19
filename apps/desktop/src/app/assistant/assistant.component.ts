import { Component, DestroyRef, inject, signal } from "@angular/core";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import { OWL } from "../owl";
import { SessionService } from "../session/session.service";
import { AssistantReply, ChatMessage } from "./assistant.model";

@Component({
  selector: "app-assistant",
  templateUrl: "./assistant.component.html",
  styleUrl: "./assistant.component.css",
})
export class AssistantComponent {
  private readonly sessions = inject(SessionService);

  readonly owl = OWL;
  readonly messages = signal<ChatMessage[]>([]);
  readonly waiting = signal(false);

  constructor() {
    const stop = listen<AssistantReply>("assistant", (event) =>
      this.receive(event.payload),
    );
    inject(DestroyRef).onDestroy(() => void stop.then((unlisten) => unlisten()));
  }

  async send(input: HTMLTextAreaElement): Promise<void> {
    const text = input.value.trim();
    if (text === "" || this.waiting()) {
      return;
    }

    input.value = "";
    this.messages.update((all) => [
      ...all,
      { from: "player", text, pending: false, failed: false },
      { from: "warden", text: "...", pending: true, failed: false },
    ]);
    this.waiting.set(true);

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
    await invoke("assistant_reset");
    this.messages.set([]);
  }

  private receive(reply: AssistantReply): void {
    this.messages.update((all) => {
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
      this.waiting.set(false);
    }
  }
}
