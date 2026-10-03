import { Component, inject, signal } from "@angular/core";

import { CatalogueEntry, InstalledPlugin } from "../session/session.model";
import { SessionService } from "../session/session.service";

type Standing = "incompatible" | "absent" | "outdated" | "current";

@Component({
  selector: "app-catalogue",
  templateUrl: "./catalogue.component.html",
  styleUrl: "./catalogue.component.css",
})
export class CatalogueComponent {
  private readonly sessions = inject(SessionService);

  readonly entries = signal<readonly CatalogueEntry[] | null>(null);
  readonly failure = signal<string | null>(null);
  readonly installing = signal<string | null>(null);
  readonly installed = signal<InstalledPlugin | null>(null);

  constructor() {
    void this.load();
  }

  async load(): Promise<void> {
    this.failure.set(null);
    try {
      this.entries.set(await this.sessions.catalogue());
    } catch (error) {
      this.failure.set(String(error));
    }
  }

  standing(entry: CatalogueEntry): Standing {
    if (entry.available === null) {
      return "incompatible";
    }
    if (entry.installed === null) {
      return "absent";
    }
    return newer(entry.available, entry.installed) ? "outdated" : "current";
  }

  async install(entry: CatalogueEntry): Promise<void> {
    if (this.installing() !== null) {
      return;
    }
    this.installing.set(entry.id);
    this.installed.set(null);
    this.failure.set(null);
    try {
      this.installed.set(await this.sessions.installPlugin(entry.id));
      await this.load();
    } catch (error) {
      this.failure.set(String(error));
    } finally {
      this.installing.set(null);
    }
  }
}

function newer(candidate: string, current: string): boolean {
  const left = candidate.split(".").map(Number);
  const right = current.split(".").map(Number);
  for (let index = 0; index < Math.max(left.length, right.length); index++) {
    const difference = (left[index] ?? 0) - (right[index] ?? 0);
    if (difference !== 0) {
      return difference > 0;
    }
  }
  return false;
}
