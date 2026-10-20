import { Component, inject } from "@angular/core";

import { OWL } from "../owl";
import { AssistantService } from "./assistant.service";

@Component({
  selector: "app-assistant",
  templateUrl: "./assistant.component.html",
  styleUrl: "./assistant.component.css",
})
export class AssistantComponent {
  private readonly assistant = inject(AssistantService);

  readonly owl = OWL;
  readonly messages = this.assistant.messages;
  readonly waiting = this.assistant.waiting;
  readonly informed = this.assistant.informed;

  inform(): void {
    this.assistant.inform();
  }

  onEnter(event: KeyboardEvent, input: HTMLTextAreaElement): void {
    if (event.shiftKey || event.isComposing) {
      return;
    }
    event.preventDefault();
    void this.send(input);
  }

  async send(input: HTMLTextAreaElement): Promise<void> {
    const text = input.value.trim();
    if (text === "" || this.waiting() || !this.informed()) {
      return;
    }
    input.value = "";
    await this.assistant.ask(text);
  }

  reset(): Promise<void> {
    return this.assistant.reset();
  }
}
