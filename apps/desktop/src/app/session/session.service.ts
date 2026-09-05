import { Injectable, signal } from "@angular/core";
import { invoke } from "@tauri-apps/api/core";

import { Command, Refused, Session, SessionEvent } from "./session.model";

@Injectable({ providedIn: "root" })
export class SessionService {
  private readonly current = signal<Session | null>(null);
  private readonly lastRefusal = signal<Refused | null>(null);
  private readonly recent = signal<readonly SessionEvent[]>([]);

  readonly session = this.current.asReadonly();
  readonly refusal = this.lastRefusal.asReadonly();
  readonly events = this.recent.asReadonly();

  /// Reading the state is what drives detection on the Rust side, so this has
  /// to keep being called rather than run once at startup.
  async refresh(): Promise<void> {
    this.current.set(await invoke<Session>("session_state"));
    const published = await invoke<SessionEvent[]>("session_events");
    if (published.length > 0) {
      this.recent.update((existing) =>
        [...published.reverse(), ...existing].slice(0, 200),
      );
    }
  }

  async engageKillSwitch(): Promise<void> {
    this.current.set(await invoke<Session>("engage_kill_switch"));
  }

  async dispatch(command: Command): Promise<void> {
    this.lastRefusal.set(null);
    try {
      this.current.set(await invoke<Session>("dispatch", { command }));
    } catch (error) {
      this.lastRefusal.set(error as Refused);
    }
  }
}
