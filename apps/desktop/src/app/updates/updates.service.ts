import { Injectable, signal } from "@angular/core";
import { invoke } from "@tauri-apps/api/core";

import { CheckResult, UpdateChannel, UpdateSettings } from "./updates.model";

@Injectable({ providedIn: "root" })
export class UpdatesService {
  private readonly current = signal<UpdateSettings | null>(null);
  private readonly lastResult = signal<CheckResult | null>(null);
  private readonly lastError = signal<string | null>(null);
  private readonly busy = signal(false);

  readonly settings = this.current.asReadonly();
  readonly result = this.lastResult.asReadonly();
  readonly error = this.lastError.asReadonly();
  readonly checking = this.busy.asReadonly();

  async refresh(): Promise<void> {
    this.current.set(await invoke<UpdateSettings>("update_settings"));
  }

  async setChannel(channel: UpdateChannel): Promise<void> {
    this.lastResult.set(null);
    this.lastError.set(null);
    try {
      this.current.set(await invoke<UpdateSettings>("set_update_channel", { channel }));
    } catch (error) {
      this.lastError.set(String(error));
    }
  }

  async check(): Promise<void> {
    this.busy.set(true);
    this.lastResult.set(null);
    this.lastError.set(null);
    try {
      this.lastResult.set(await invoke<CheckResult>("check_for_update"));
    } catch (error) {
      this.lastError.set(String(error));
    } finally {
      this.busy.set(false);
    }
  }
}
