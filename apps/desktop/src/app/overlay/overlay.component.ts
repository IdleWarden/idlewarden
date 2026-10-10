import { Component, OnDestroy, computed, signal } from "@angular/core";
import { invoke } from "@tauri-apps/api/core";
import { UnlistenFn, listen } from "@tauri-apps/api/event";

import { OWL } from "../owl";
import { describe, showValue } from "../session/blockers";
import {
  Command,
  Observation,
  PluginSummary,
  Refused,
  Session,
  Signal,
  Unmet,
} from "../session/session.model";
import { OverlaySettings } from "./overlay.model";

const POLL_MS = 500;

@Component({
  selector: "app-root",
  templateUrl: "./overlay.component.html",
  styleUrl: "./overlay.component.css",
})
export class OverlayComponent implements OnDestroy {
  readonly owl = OWL;
  readonly session = signal<Session | null>(null);
  readonly plugins = signal<readonly PluginSummary[]>([]);
  readonly settings = signal<OverlaySettings | null>(null);
  readonly expanded = signal(false);
  readonly observation = signal<Observation | null>(null);
  readonly signals = computed(() => this.observation()?.signals ?? []);
  readonly refusal = signal<string | null>(null);
  readonly game = computed(
    () => this.plugins().find((plugin) => plugin.detected) ?? null,
  );

  private readonly timer = setInterval(() => void this.refresh(), POLL_MS);
  private unlisten: UnlistenFn | null = null;

  constructor() {
    void this.refresh();
    void invoke<OverlaySettings>("overlay_settings").then((settings) =>
      this.settings.set(settings),
    );
    void listen<boolean>("overlay-expanded", (event) =>
      this.expanded.set(event.payload),
    ).then((unlisten) => (this.unlisten = unlisten));
  }

  ngOnDestroy(): void {
    clearInterval(this.timer);
    this.unlisten?.();
  }

  private async refresh(): Promise<void> {
    this.session.set(await invoke<Session>("session_state"));
    this.plugins.set(await invoke<PluginSummary[]>("plugins"));
    this.observation.set(await invoke<Observation | null>("session_observation"));
  }

  value(signal: Signal): string {
    return showValue(signal.value);
  }

  reasons(unmet: readonly Unmet[]): string {
    return unmet.map(describe).join(", ");
  }

  async expand(expanded: boolean): Promise<void> {
    this.expanded.set(expanded);
    if (expanded) {
      this.settings.set(await invoke<OverlaySettings>("overlay_settings"));
    }
    await invoke("set_overlay_expanded", { expanded });
  }

  watchLabel(session: Session): string {
    switch (session.state) {
      case "running":
        return "Pause";
      case "paused":
        return "Reprendre";
      default:
        return "Démarrer";
    }
  }

  stateLabel(session: Session): string {
    switch (session.state) {
      case "searching":
        return "aucun jeu";
      case "ready":
        return "prêt";
      case "running":
        return "en veille";
      case "paused":
        return session.last_reason ?? "en pause";
      case "halted":
        return "arrêté";
    }
  }

  async toggleWatch(session: Session): Promise<void> {
    if (session.state === "running") {
      await this.dispatch({ command: "pause" });
    } else if (session.state === "paused") {
      await this.dispatch({ command: "resume" });
    } else if (session.plugin !== null) {
      await this.dispatch({
        command: "start",
        plugin: session.plugin,
        profile: "default",
      });
    }
  }

  async stop(): Promise<void> {
    await this.dispatch({ command: "stop" });
  }

  async setDryRun(enabled: boolean): Promise<void> {
    await this.dispatch({ command: "set_dry_run", enabled });
  }

  async killSwitch(): Promise<void> {
    this.session.set(await invoke<Session>("engage_kill_switch"));
  }

  async toggleIntent(plugin: string, intent: string, enabled: boolean): Promise<void> {
    this.plugins.set(
      await invoke<PluginSummary[]>("set_intent_enabled", { plugin, intent, enabled }),
    );
  }

  private async dispatch(command: Command): Promise<void> {
    this.refusal.set(null);
    try {
      this.session.set(await invoke<Session>("dispatch", { command }));
    } catch (error) {
      this.refusal.set((error as Refused).message);
    }
  }
}
