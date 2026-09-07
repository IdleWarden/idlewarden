import { Component, computed, inject, signal } from "@angular/core";

import { LogLevel, LogRecord } from "../session/session.model";
import { SessionService } from "../session/session.service";

const ORDER: readonly LogLevel[] = ["ERROR", "WARN", "INFO", "DEBUG", "TRACE"];

@Component({
  selector: "app-logs",
  templateUrl: "./logs.component.html",
  styleUrl: "./logs.component.css",
})
export class LogsComponent {
  private readonly sessions = inject(SessionService);

  readonly levels = ORDER;
  readonly floor = signal<LogLevel>("DEBUG");
  readonly target = signal("");

  readonly targets = computed(() => {
    const seen = new Set(this.sessions.logs().map((record) => record.target));
    return [...seen].sort();
  });

  readonly shown = computed(() => {
    const depth = ORDER.indexOf(this.floor());
    const needle = this.target().toLowerCase();
    return this.sessions
      .logs()
      .filter((record) => ORDER.indexOf(record.level) <= depth)
      .filter(
        (record) => needle === "" || record.target.toLowerCase().includes(needle),
      );
  });

  setFloor(level: LogLevel): void {
    this.floor.set(level);
  }

  setTarget(value: string): void {
    this.target.set(value);
  }

  clock(at_ms: number): string {
    return new Date(at_ms).toLocaleTimeString();
  }

  /// Fields stay separate values rather than one formatted line, so a reader
  /// can see which of them carried the surprise.
  entries(record: LogRecord): { key: string; value: string }[] {
    return Object.entries(record.fields).map(([key, value]) => ({
      key,
      value: typeof value === "string" ? value : JSON.stringify(value),
    }));
  }
}
