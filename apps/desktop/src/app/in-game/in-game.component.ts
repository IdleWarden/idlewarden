import { Component, signal } from "@angular/core";
import { invoke } from "@tauri-apps/api/core";

import { Corner, Hotkeys, OverlaySettings } from "../overlay/overlay.model";

type HotkeyAction = keyof Hotkeys;

const MODIFIER_KEYS = new Set(["Control", "Shift", "Alt", "Meta"]);

@Component({
  selector: "app-in-game",
  templateUrl: "./in-game.component.html",
  styleUrl: "./in-game.component.css",
})
export class InGameComponent {
  readonly settings = signal<OverlaySettings | null>(null);
  readonly error = signal<string | null>(null);
  readonly recording = signal<HotkeyAction | null>(null);

  readonly corners: readonly { value: Corner; label: string }[] = [
    { value: "top_left", label: "Haut gauche" },
    { value: "top_right", label: "Haut droite" },
    { value: "bottom_left", label: "Bas gauche" },
    { value: "bottom_right", label: "Bas droite" },
  ];

  readonly actions: readonly { key: HotkeyAction; label: string; required: boolean }[] =
    [
      {
        key: "toggle_watch",
        label: "Démarrer, mettre en pause, reprendre",
        required: false,
      },
      { key: "toggle_panel", label: "Ouvrir ou réduire le panneau", required: false },
      {
        key: "toggle_overlay",
        label: "Afficher ou masquer l'overlay",
        required: false,
      },
      { key: "kill_switch", label: "Coupe-circuit", required: true },
    ];

  constructor() {
    void invoke<OverlaySettings>("overlay_settings").then((settings) =>
      this.settings.set(settings),
    );
  }

  async save(change: Partial<OverlaySettings>): Promise<void> {
    const current = this.settings();
    if (current === null) {
      return;
    }
    this.error.set(null);
    try {
      this.settings.set(
        await invoke<OverlaySettings>("set_overlay_settings", {
          settings: { ...current, ...change },
        }),
      );
    } catch (error) {
      this.error.set(String(error));
    }
  }

  margin(axis: "margin_x" | "margin_y", value: string): void {
    const parsed = Number.parseInt(value, 10);
    if (Number.isFinite(parsed)) {
      void this.save({ [axis]: parsed });
    }
  }

  record(action: HotkeyAction): void {
    this.recording.set(this.recording() === action ? null : action);
  }

  async captured(event: KeyboardEvent, action: HotkeyAction): Promise<void> {
    if (this.recording() !== action || MODIFIER_KEYS.has(event.key)) {
      return;
    }
    event.preventDefault();
    this.recording.set(null);
    if (event.key !== "Escape") {
      await this.bind(action, combination(event));
    }
  }

  async bind(action: HotkeyAction, keys: string): Promise<void> {
    const current = this.settings();
    if (current !== null) {
      await this.save({ hotkeys: { ...current.hotkeys, [action]: keys } });
    }
  }
}

function combination(event: KeyboardEvent): string {
  return [
    event.ctrlKey ? "Ctrl" : null,
    event.altKey ? "Alt" : null,
    event.shiftKey ? "Shift" : null,
    event.metaKey ? "Super" : null,
    event.code,
  ]
    .filter((part) => part !== null)
    .join("+");
}
