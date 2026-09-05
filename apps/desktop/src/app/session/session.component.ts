import { Component, OnDestroy, OnInit, computed, inject } from "@angular/core";

import { UpdatesComponent } from "../updates/updates.component";
import { SessionEvent } from "./session.model";
import { SessionService } from "./session.service";

const POLL_MS = 500;

@Component({
  selector: "app-session",
  imports: [UpdatesComponent],
  templateUrl: "./session.component.html",
  styleUrl: "./session.component.css",
})
export class SessionComponent implements OnInit, OnDestroy {
  private readonly sessions = inject(SessionService);

  readonly session = this.sessions.session;
  readonly refusal = this.sessions.refusal;
  readonly plugin = computed(() => this.session()?.plugin ?? null);
  readonly events = this.sessions.events;

  private timer: ReturnType<typeof setInterval> | null = null;

  ngOnInit(): void {
    void this.sessions.refresh();
    this.timer = setInterval(() => void this.sessions.refresh(), POLL_MS);
  }

  ngOnDestroy(): void {
    if (this.timer !== null) {
      clearInterval(this.timer);
    }
  }

  killSwitch(): void {
    void this.sessions.engageKillSwitch();
  }

  describe(event: SessionEvent): string {
    switch (event.event) {
      case "game_detected":
        return `detected ${event.plugin} in "${event.window_title}"`;
      case "game_lost":
        return "the game window is gone";
      case "plugin_loaded":
        return `loaded ${event.plugin}`;
      case "plugin_failed":
        return `${event.plugin} failed: ${event.reason}`;
      case "intent_proposed":
        return `wants to ${event.intent.name}`;
      case "intent_rejected":
        return `refused ${event.intent.name}: ${event.reason}`;
      case "action_started":
        return `doing ${event.intent.name}`;
      case "action_finished":
        return `finished ${event.intent.name}`;
      case "agent_paused":
        return `paused: ${event.reason}`;
      case "agent_resumed":
        return "resumed";
      case "kill_switch":
        return "kill switch engaged";
      case "error":
        return event.message;
      default:
        return event.event;
    }
  }

  start(): void {
    const plugin = this.plugin();
    if (plugin === null) {
      return;
    }
    void this.sessions.dispatch({ command: "start", plugin, profile: "default" });
  }

  stop(): void {
    void this.sessions.dispatch({ command: "stop" });
  }

  pause(): void {
    void this.sessions.dispatch({ command: "pause" });
  }

  resume(): void {
    void this.sessions.dispatch({ command: "resume" });
  }

  toggleDryRun(): void {
    const session = this.session();
    if (session === null) {
      return;
    }
    void this.sessions.dispatch({
      command: "set_dry_run",
      enabled: !session.dry_run,
    });
  }
}
