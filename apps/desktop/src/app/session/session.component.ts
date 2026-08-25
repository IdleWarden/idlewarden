import { Component, OnInit, computed, inject } from "@angular/core";

import { SessionService } from "./session.service";

@Component({
  selector: "app-session",
  imports: [],
  templateUrl: "./session.component.html",
  styleUrl: "./session.component.css",
})
export class SessionComponent implements OnInit {
  private readonly sessions = inject(SessionService);

  readonly session = this.sessions.session;
  readonly refusal = this.sessions.refusal;
  readonly plugin = computed(() => this.session()?.plugin ?? null);

  ngOnInit(): void {
    void this.sessions.refresh();
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
