import { Component, computed, inject } from "@angular/core";
import { RouterOutlet } from "@angular/router";

import { OWL } from "./owl";
import { PluginSummary } from "./session/session.model";
import { SessionService } from "./session/session.service";

@Component({
  selector: "app-root",
  imports: [RouterOutlet],
  templateUrl: "./app.component.html",
  styleUrl: "./app.component.css",
})
export class AppComponent {
  private readonly sessions = inject(SessionService);

  readonly plugins = this.sessions.plugins;
  readonly owl = computed(() => OWL);

  stateOf(plugin: PluginSummary): string {
    if (!plugin.detected) {
      return "en attente";
    }
    return this.sessions.session()?.state === "running" ? "en veille" : "prêt";
  }
}
