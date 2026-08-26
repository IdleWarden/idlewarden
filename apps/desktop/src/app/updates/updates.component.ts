import { Component, OnInit, inject } from "@angular/core";

import { UpdateChannel } from "./updates.model";
import { UpdatesService } from "./updates.service";

@Component({
  selector: "app-updates",
  imports: [],
  templateUrl: "./updates.component.html",
  styleUrl: "./updates.component.css",
})
export class UpdatesComponent implements OnInit {
  private readonly updates = inject(UpdatesService);

  readonly settings = this.updates.settings;
  readonly result = this.updates.result;
  readonly error = this.updates.error;
  readonly checking = this.updates.checking;

  readonly channels: UpdateChannel[] = ["stable", "beta"];

  ngOnInit(): void {
    void this.updates.refresh();
  }

  select(channel: UpdateChannel): void {
    void this.updates.setChannel(channel);
  }

  check(): void {
    void this.updates.check();
  }
}
