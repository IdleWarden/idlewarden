import { Component, computed, inject } from "@angular/core";
import { RouterLink, RouterLinkActive, RouterOutlet } from "@angular/router";

import { OWL } from "./owl";
import { PluginSummary } from "./session/session.model";
import { SessionService } from "./session/session.service";
import { WindowControlsComponent } from "./window-controls/window-controls.component";

@Component({
  selector: "app-root",
  imports: [RouterLink, RouterLinkActive, RouterOutlet, WindowControlsComponent],
  templateUrl: "./app.component.html",
  styleUrl: "./app.component.css",
})
export class AppComponent {
  private readonly sessions = inject(SessionService);

  readonly plugins = this.sessions.plugins;
  readonly owl = computed(() => OWL);
  readonly screens = [
    { path: "/", label: "Session" },
    { path: "/detection", label: "Détection" },
    { path: "/activite", label: "Activité" },
    { path: "/journaux", label: "Journaux" },
    { path: "/profils", label: "Profils" },
    { path: "/editeur", label: "Éditeur" },
  ];

  stateOf(plugin: PluginSummary): string {
    if (!plugin.detected) {
      return "en attente";
    }
    return this.sessions.session()?.state === "running" ? "en veille" : "prêt";
  }
}
