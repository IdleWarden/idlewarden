import { Injectable, signal } from "@angular/core";
import { invoke } from "@tauri-apps/api/core";

import {
  Command,
  Observation,
  PluginSummary,
  Refused,
  Session,
  SessionEvent,
} from "./session.model";

@Injectable({ providedIn: "root" })
export class SessionService {
  private readonly current = signal<Session | null>(null);
  private readonly lastRefusal = signal<Refused | null>(null);
  private readonly recent = signal<readonly SessionEvent[]>([]);
  private readonly known = signal<readonly PluginSummary[]>([]);
  private readonly seen = signal<Observation | null>(null);

  readonly session = this.current.asReadonly();
  readonly refusal = this.lastRefusal.asReadonly();
  readonly events = this.recent.asReadonly();
  readonly plugins = this.known.asReadonly();
  readonly observation = this.seen.asReadonly();

  /// Reading the state is what drives detection on the Rust side, so this has
  /// to keep being called rather than run once at startup.
  async refresh(): Promise<void> {
    this.current.set(await invoke<Session>("session_state"));
    await this.refreshPlugins();
    const published = await invoke<SessionEvent[]>("session_events");
    if (published.length > 0) {
      const observed = published.filter((event) => event.event === "observed");
      const latest = observed[observed.length - 1];
      if (latest !== undefined && latest.event === "observed") {
        this.seen.set(latest.observation);
      }
      this.recent.update((existing) =>
        [...published.reverse(), ...existing].slice(0, 200),
      );
    }
  }

  async refreshPlugins(): Promise<void> {
    this.known.set(await invoke<PluginSummary[]>("plugins"));
  }

  async setIntentEnabled(
    plugin: string,
    intent: string,
    enabled: boolean,
  ): Promise<void> {
    this.known.set(
      await invoke<PluginSummary[]>("set_intent_enabled", {
        plugin,
        intent,
        enabled,
      }),
    );
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
