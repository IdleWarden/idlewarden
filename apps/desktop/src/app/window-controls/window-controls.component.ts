import { Component, DestroyRef, inject, signal } from "@angular/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

@Component({
  selector: "app-window-controls",
  templateUrl: "./window-controls.component.html",
  styleUrl: "./window-controls.component.css",
})
export class WindowControlsComponent {
  private readonly window = getCurrentWindow();

  readonly maximized = signal(false);
  readonly cross = Array.from({ length: 10 }, (_, index) => index);

  constructor() {
    void this.refresh();
    const stopListening = this.window.onResized(() => void this.refresh());
    inject(DestroyRef).onDestroy(() => void stopListening.then((stop) => stop()));
  }

  minimize(): void {
    void this.window.minimize();
  }

  toggleMaximize(): void {
    void this.window.toggleMaximize();
  }

  close(): void {
    void this.window.close();
  }

  private async refresh(): Promise<void> {
    this.maximized.set(await this.window.isMaximized());
  }
}
