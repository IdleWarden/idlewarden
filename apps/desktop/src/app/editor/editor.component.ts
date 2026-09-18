import { Component, OnDestroy, computed, inject, signal } from "@angular/core";

import {
  Draft,
  Region,
  RegionKind,
  Roi,
  WindowCandidate,
} from "../session/session.model";
import { SessionService } from "../session/session.service";

interface Point {
  x: number;
  y: number;
}

const SMALLEST_SIDE = 0.005;

const KINDS: readonly { key: RegionKind; label: string }[] = [
  { key: "color_probe", label: "Sonde de couleur" },
  { key: "template_match", label: "Motif" },
  { key: "anchor", label: "Ancre" },
];

@Component({
  selector: "app-editor",
  templateUrl: "./editor.component.html",
  styleUrl: "./editor.component.css",
})
export class EditorComponent implements OnDestroy {
  private readonly sessions = inject(SessionService);

  readonly kinds = KINDS;
  readonly windows = this.sessions.candidates;
  readonly chosen = signal<WindowCandidate | null>(null);
  readonly image = signal<string | null>(null);
  readonly capturing = signal(false);
  readonly regions = signal<readonly Region[]>([]);
  readonly pending = signal<Roi | null>(null);
  readonly id = signal("");
  readonly name = signal("");
  readonly executable = signal("");
  readonly outcome = signal<{ ok: boolean; message: string } | null>(null);

  readonly ready = computed(
    () =>
      this.image() !== null &&
      this.regions().length > 0 &&
      this.id() !== "" &&
      this.name().trim() !== "" &&
      this.executable().trim() !== "",
  );

  private start: Point | null = null;

  choose(handle: string): void {
    const candidate =
      this.windows().find((entry) => String(entry.handle) === handle) ?? null;
    this.chosen.set(candidate);
    if (candidate === null) {
      return;
    }
    this.executable.set(candidate.executable);
    this.name.set(candidate.title);
    this.id.set(suggestId(candidate.executable));
  }

  async capture(): Promise<void> {
    const candidate = this.chosen();
    if (candidate === null) {
      return;
    }
    this.capturing.set(true);
    this.outcome.set(null);
    try {
      const png = await this.sessions.captureFrame(candidate.handle);
      this.replaceImage(URL.createObjectURL(new Blob([png], { type: "image/png" })));
      this.regions.set([]);
    } catch (error) {
      this.outcome.set({ ok: false, message: String(error) });
    } finally {
      this.capturing.set(false);
    }
  }

  press(event: PointerEvent): void {
    this.start = point(event);
    (event.currentTarget as Element).setPointerCapture(event.pointerId);
  }

  drag(event: PointerEvent): void {
    if (this.start !== null) {
      this.pending.set(rect(this.start, point(event)));
    }
  }

  release(event: PointerEvent): void {
    if (this.start === null) {
      return;
    }
    const area = rect(this.start, point(event));
    this.start = null;
    this.pending.set(null);
    if (area.w < SMALLEST_SIDE || area.h < SMALLEST_SIDE) {
      return;
    }
    this.regions.update((existing) => [
      ...existing,
      { name: nextName(existing), kind: "color_probe", area },
    ]);
  }

  rename(index: number, name: string): void {
    this.update(index, { name: name.trim().toLowerCase() });
  }

  retype(index: number, kind: RegionKind): void {
    this.update(index, { kind });
  }

  remove(index: number): void {
    this.regions.update((existing) => existing.filter((_, at) => at !== index));
  }

  async save(): Promise<void> {
    const draft: Draft = {
      id: this.id().trim(),
      name: this.name().trim(),
      game: { executable: this.executable().trim() },
      regions: [...this.regions()],
    };
    try {
      const path = await this.sessions.savePlugin(draft);
      this.outcome.set({ ok: true, message: `Plugin écrit dans ${path}` });
    } catch (error) {
      this.outcome.set({ ok: false, message: String(error) });
    }
  }

  label(kind: RegionKind): string {
    return KINDS.find((entry) => entry.key === kind)?.label ?? kind;
  }

  percent(value: number): string {
    return `${value * 100}%`;
  }

  ngOnDestroy(): void {
    this.replaceImage(null);
  }

  private update(index: number, change: Partial<Region>): void {
    this.regions.update((existing) =>
      existing.map((region, at) => (at === index ? { ...region, ...change } : region)),
    );
  }

  private replaceImage(url: string | null): void {
    const previous = this.image();
    if (previous !== null) {
      URL.revokeObjectURL(previous);
    }
    this.image.set(url);
  }
}

function point(event: PointerEvent): Point {
  const bounds = (event.currentTarget as Element).getBoundingClientRect();
  return {
    x: clamp((event.clientX - bounds.left) / bounds.width),
    y: clamp((event.clientY - bounds.top) / bounds.height),
  };
}

function clamp(value: number): number {
  return Math.min(1, Math.max(0, value));
}

function rect(a: Point, b: Point): Roi {
  return {
    x: Math.min(a.x, b.x),
    y: Math.min(a.y, b.y),
    w: Math.abs(a.x - b.x),
    h: Math.abs(a.y - b.y),
  };
}

function nextName(existing: readonly Region[]): string {
  let index = existing.length + 1;
  while (existing.some((region) => region.name === `zone-${index}`)) {
    index += 1;
  }
  return `zone-${index}`;
}

function suggestId(executable: string): string {
  const slug = executable
    .replace(/\.exe$/i, "")
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
  return `local.${slug === "" ? "game" : slug}`;
}
