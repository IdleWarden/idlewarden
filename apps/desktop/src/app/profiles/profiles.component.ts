import { Component, computed, effect, inject, signal } from "@angular/core";

import { Profile } from "../session/session.model";
import { SessionService } from "../session/session.service";

interface Limit {
  key:
    | "max_actions_per_minute"
    | "min_confidence"
    | "max_observation_age_ms"
    | "max_session_minutes";
  label: string;
  hint: string;
  min: number;
  max: number;
  step: number;
  unit: string;
}

const LIMITS: readonly Limit[] = [
  {
    key: "max_actions_per_minute",
    label: "Plafond d'actions",
    hint: "Un jeu idle n'a pas besoin de plus de quelques actions par minute.",
    min: 1,
    max: 600,
    step: 1,
    unit: "par minute",
  },
  {
    key: "min_confidence",
    label: "Seuil de confiance",
    hint: "En dessous, le Gardien s'arrête plutôt que de deviner.",
    min: 0.05,
    max: 1,
    step: 0.05,
    unit: "",
  },
  {
    key: "max_observation_age_ms",
    label: "Âge maximal d'une observation",
    hint: "Une image plus vieille que ça n'est plus une base pour agir.",
    min: 100,
    max: 60000,
    step: 100,
    unit: "ms",
  },
  {
    key: "max_session_minutes",
    label: "Budget de session",
    hint: "Au-delà, la veille s'arrête d'elle-même.",
    min: 1,
    max: 1440,
    step: 10,
    unit: "minutes",
  },
];

@Component({
  selector: "app-profiles",
  templateUrl: "./profiles.component.html",
  styleUrl: "./profiles.component.css",
})
export class ProfilesComponent {
  private readonly sessions = inject(SessionService);

  readonly limits = LIMITS;
  readonly plugins = this.sessions.plugins;
  readonly chosen = signal<string | null>(null);
  readonly saved = signal(false);

  readonly plugin = computed(() => {
    const chosen = this.chosen();
    const known = this.plugins();
    if (chosen !== null && known.some((entry) => entry.id === chosen)) {
      return chosen;
    }
    return known.find((entry) => entry.detected)?.id ?? known[0]?.id ?? null;
  });

  readonly intents = computed(
    () => this.plugins().find((entry) => entry.id === this.plugin())?.intents ?? [],
  );

  readonly profile = signal<Profile | null>(null);

  /// The limits follow whichever plugin is selected, including the first time
  /// detection names one.
  constructor() {
    effect(() => {
      const plugin = this.plugin();
      if (plugin === null) {
        this.profile.set(null);
        return;
      }
      void this.sessions.profile(plugin).then((loaded) => this.profile.set(loaded));
    });
  }

  choose(plugin: string): void {
    this.chosen.set(plugin);
    this.saved.set(false);
  }

  edit(key: Limit["key"], raw: string): void {
    const current = this.profile();
    if (current === null) {
      return;
    }
    const value = Number(raw);
    if (!Number.isFinite(value)) {
      return;
    }
    this.profile.set({ ...current, [key]: value });
    this.saved.set(false);
  }

  value(limit: Limit): number {
    return this.profile()?.[limit.key] ?? 0;
  }

  /// The Rust side clamps what it stores, so the answer it returns is the
  /// truth, not the numbers that were typed.
  async save(): Promise<void> {
    const plugin = this.plugin();
    const edited = this.profile();
    if (plugin === null || edited === null) {
      return;
    }
    this.profile.set(await this.sessions.saveProfile(plugin, edited));
    this.saved.set(true);
  }

  toggleIntent(intent: string, enabled: boolean): void {
    const plugin = this.plugin();
    if (plugin === null) {
      return;
    }
    void this.sessions.setIntentEnabled(plugin, intent, enabled);
  }
}
