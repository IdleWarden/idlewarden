import { Component, computed, inject, signal } from "@angular/core";

import { ActionOutcome, PublishedEvent } from "../session/session.model";
import { SessionService } from "../session/session.service";

type ActivityFilter = "all" | "intents" | "refusals" | "trouble";

/// What a row means, which drives its colour and its place in a filter.
type Grade = "proposed" | "allowed" | "refused" | "failed" | "context";

interface ActivityRow {
  at_ms: number;
  grade: Grade;
  intent: string | null;
  what: string;
  detail: string | null;
}

const INTENT_GRADES: readonly Grade[] = ["proposed", "allowed", "refused", "failed"];

@Component({
  selector: "app-activity",
  templateUrl: "./activity.component.html",
  styleUrl: "./activity.component.css",
})
export class ActivityComponent {
  private readonly sessions = inject(SessionService);

  readonly filter = signal<ActivityFilter>("all");
  readonly filters: readonly { key: ActivityFilter; label: string }[] = [
    { key: "all", label: "Tout" },
    { key: "intents", label: "Intentions" },
    { key: "refusals", label: "Refus" },
    { key: "trouble", label: "Incidents" },
  ];

  readonly rows = computed(() => this.sessions.events().map((event) => row(event)));

  readonly shown = computed(() => {
    const rows = this.rows();
    switch (this.filter()) {
      case "all":
        return rows;
      case "intents":
        return rows.filter((entry) => INTENT_GRADES.includes(entry.grade));
      case "refusals":
        return rows.filter((entry) => entry.grade === "refused");
      case "trouble":
        return rows.filter((entry) => entry.grade === "failed");
    }
  });

  readonly refusalCount = computed(
    () => this.rows().filter((entry) => entry.grade === "refused").length,
  );

  setFilter(filter: ActivityFilter): void {
    this.filter.set(filter);
  }

  clock(at_ms: number): string {
    return new Date(at_ms).toLocaleTimeString();
  }
}

function outcome(result: ActionOutcome): {
  grade: Grade;
  what: string;
  detail: string | null;
} {
  switch (result.outcome) {
    case "succeeded":
      return { grade: "allowed", what: "terminé", detail: null };
    case "failed":
      return { grade: "failed", what: "échoué", detail: result.reason };
    case "rejected":
      return { grade: "failed", what: "refusé à l'exécution", detail: result.reason };
    case "aborted":
      return { grade: "failed", what: "interrompu", detail: null };
    case "timed_out":
      return { grade: "failed", what: "expiré", detail: `après ${result.after_ms} ms` };
  }
}

/// One event becomes one row. Rejected intents are rows like any other, which
/// is the point of the screen: an agent that is refusing to act looks identical
/// to an idle one unless the refusals are visible.
function row(event: PublishedEvent): ActivityRow {
  const at_ms = event.at_ms;

  switch (event.event) {
    case "intent_proposed":
      return {
        at_ms,
        grade: "proposed",
        intent: event.intent.name,
        what: "proposé",
        detail: null,
      };
    case "intent_rejected":
      return {
        at_ms,
        grade: "refused",
        intent: event.intent.name,
        what: "refusé par le Gouverneur",
        detail: event.reason,
      };
    case "action_started":
      return {
        at_ms,
        grade: "allowed",
        intent: event.intent.name,
        what: "exécution",
        detail: null,
      };
    case "action_finished": {
      const result = outcome(event.outcome);
      return { at_ms, intent: event.intent.name, ...result };
    }
    case "agent_paused":
      return {
        at_ms,
        grade: "failed",
        intent: null,
        what: "agent en pause",
        detail: event.reason,
      };
    case "agent_resumed":
      return {
        at_ms,
        grade: "context",
        intent: null,
        what: "agent repris",
        detail: null,
      };
    case "kill_switch":
      return {
        at_ms,
        grade: "failed",
        intent: null,
        what: "coupe-circuit",
        detail: null,
      };
    case "error":
      return {
        at_ms,
        grade: "failed",
        intent: null,
        what: "erreur",
        detail: event.message,
      };
    case "game_detected":
      return {
        at_ms,
        grade: "context",
        intent: null,
        what: "jeu détecté",
        detail: `${event.plugin} · ${event.window_title}`,
      };
    case "game_lost":
      return { at_ms, grade: "context", intent: null, what: "jeu perdu", detail: null };
    case "plugin_loaded":
      return {
        at_ms,
        grade: "context",
        intent: null,
        what: "plugin chargé",
        detail: event.plugin,
      };
    case "plugin_failed":
      return {
        at_ms,
        grade: "failed",
        intent: null,
        what: "plugin en échec",
        detail: `${event.plugin} · ${event.reason}`,
      };
    case "observed":
      return {
        at_ms,
        grade: "context",
        intent: null,
        what: "observation",
        detail: `${event.observation.signals.length} signaux`,
      };
  }
}
