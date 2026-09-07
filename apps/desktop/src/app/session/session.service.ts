import { Injectable, signal } from "@angular/core";
import { invoke } from "@tauri-apps/api/core";

import {
  Command,
  LogRecord,
  Observation,
  PluginSummary,
  Profile,
  PublishedEvent,
  Refused,
  Session,
  WindowCandidate,
} from "./session.model";

const POLL_MS = 500;

/// How many published events the app keeps. The Rust side caps its own buffer;
/// this is the second half of the same decision, for a window left open for
/// days.
const MAX_EVENTS = 2000;

@Injectable({ providedIn: "root" })
export class SessionService {
  private readonly current = signal<Session | null>(null);
  private readonly lastRefusal = signal<Refused | null>(null);
  private readonly recent = signal<readonly PublishedEvent[]>([]);
  private readonly known = signal<readonly PluginSummary[]>([]);
  private readonly seen = signal<Observation | null>(null);
  private readonly windows = signal<readonly WindowCandidate[]>([]);
  private readonly buffered = signal<readonly LogRecord[]>([]);

  readonly session = this.current.asReadonly();
  readonly refusal = this.lastRefusal.asReadonly();
  readonly events = this.recent.asReadonly();
  readonly plugins = this.known.asReadonly();
  readonly observation = this.seen.asReadonly();
  readonly candidates = this.windows.asReadonly();
  readonly logs = this.buffered.asReadonly();

  /// Polling lives here rather than in a screen because events are drained on
  /// read: a screen that owns the timer stops draining the moment the user
  /// navigates away, and the Activity timeline loses everything that happened
  /// while they were elsewhere.
  constructor() {
    void this.tick();
    setInterval(() => void this.tick(), POLL_MS);
  }

  private async tick(): Promise<void> {
    try {
      await this.refresh();
      await this.refreshCandidates();
      await this.refreshLogs();
    } catch {
      // Outside the Tauri shell there is no bridge to talk to. Nothing can be
      // done about it and saying so 120 times a minute helps nobody.
    }
  }

  /// Reading the state is what drives detection on the Rust side, so this has
  /// to keep being called rather than run once at startup.
  async refresh(): Promise<void> {
    this.current.set(await invoke<Session>("session_state"));
    await this.refreshPlugins();
    const published = await invoke<PublishedEvent[]>("session_events");
    if (published.length > 0) {
      const observed = published.filter((event) => event.event === "observed");
      const latest = observed[observed.length - 1];
      if (latest !== undefined && latest.event === "observed") {
        this.seen.set(latest.observation);
      }
      this.recent.update((existing) =>
        [...published.reverse(), ...existing].slice(0, MAX_EVENTS),
      );
    }
  }

  async refreshCandidates(): Promise<void> {
    this.windows.set(await invoke<WindowCandidate[]>("window_candidates"));
  }

  async refreshLogs(): Promise<void> {
    const drained = await invoke<LogRecord[]>("drain_logs");
    if (drained.length > 0) {
      this.buffered.update((existing) =>
        [...drained.reverse(), ...existing].slice(0, MAX_EVENTS),
      );
    }
  }

  async refreshPlugins(): Promise<void> {
    this.known.set(await invoke<PluginSummary[]>("plugins"));
  }

  async profile(plugin: string): Promise<Profile> {
    return await invoke<Profile>("profile", { plugin });
  }

  /// Returns what was stored, not what was sent: the Rust side clamps limits
  /// that would remove a check rather than loosen it.
  async saveProfile(plugin: string, profile: Profile): Promise<Profile> {
    return await invoke<Profile>("set_profile", { plugin, profile });
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
