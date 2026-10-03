import { Component, computed, inject } from "@angular/core";
import { Router } from "@angular/router";

import { WindowCandidate } from "../session/session.model";
import { SessionService } from "../session/session.service";

@Component({
  selector: "app-detect",
  templateUrl: "./detect.component.html",
  styleUrl: "./detect.component.css",
})
export class DetectComponent {
  private readonly sessions = inject(SessionService);
  private readonly router = inject(Router);

  readonly candidates = this.sessions.candidates;
  readonly claimed = computed(() =>
    this.candidates().filter((candidate) => candidate.plugins.length > 0),
  );
  readonly unclaimed = computed(() =>
    this.candidates().filter((candidate) => candidate.plugins.length === 0),
  );

  verdict(candidate: WindowCandidate): string {
    if (candidate.plugins.length === 0) {
      return "aucun plugin";
    }
    if (candidate.plugins.length === 1) {
      return candidate.plugins[0];
    }
    return `${candidate.plugins.length} plugins revendiquent cette fenêtre`;
  }

  async start(plugin: string): Promise<void> {
    await this.sessions.dispatch({ command: "start", plugin, profile: "default" });
    if (this.sessions.refusal() === null) {
      await this.router.navigateByUrl("/");
    }
  }

  ambiguous(candidate: WindowCandidate): boolean {
    return candidate.plugins.length > 1;
  }
}
