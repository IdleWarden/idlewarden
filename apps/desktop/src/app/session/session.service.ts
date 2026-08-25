import { Injectable, signal } from "@angular/core";
import { invoke } from "@tauri-apps/api/core";

import { Command, Refused, Session } from "./session.model";

@Injectable({ providedIn: "root" })
export class SessionService {
  private readonly current = signal<Session | null>(null);
  private readonly lastRefusal = signal<Refused | null>(null);

  readonly session = this.current.asReadonly();
  readonly refusal = this.lastRefusal.asReadonly();

  async refresh(): Promise<void> {
    this.current.set(await invoke<Session>("session_state"));
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
