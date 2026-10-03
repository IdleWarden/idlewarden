import { provideZoneChangeDetection } from "@angular/core";
import { bootstrapApplication } from "@angular/platform-browser";
import { getCurrentWindow } from "@tauri-apps/api/window";

import { AppComponent } from "./app/app.component";
import { appConfig } from "./app/app.config";
import { OverlayComponent } from "./app/overlay/overlay.component";

function windowLabel(): string {
  try {
    return getCurrentWindow().label;
  } catch {
    return "main";
  }
}

if (windowLabel() === "overlay") {
  document.documentElement.classList.add("overlay");
  bootstrapApplication(OverlayComponent, {
    providers: [provideZoneChangeDetection({ eventCoalescing: true })],
  }).catch((err) => console.error(err));
} else {
  bootstrapApplication(AppComponent, appConfig).catch((err) => console.error(err));
}
