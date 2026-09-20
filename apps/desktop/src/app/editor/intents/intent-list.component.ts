import { Component, computed, input, model } from "@angular/core";

import { Condition, ConditionOp, Point } from "../../session/session.model";

export interface IntentState {
  name: string;
  when: Condition[];
  click: Point | null;
  post_condition: Condition[];
}

type Clause = "when" | "post_condition";

@Component({
  selector: "app-intent-list",
  templateUrl: "./intent-list.component.html",
  styleUrl: "./intent-list.component.css",
})
export class IntentListComponent {
  readonly intents = model.required<IntentState[]>();
  readonly picking = model<number | null>(null);
  readonly signals = input.required<string[]>();

  readonly clauses: readonly { key: Clause; label: string }[] = [
    { key: "when", label: "Quand" },
    { key: "post_condition", label: "Ensuite, vérifier que" },
  ];

  readonly ops: readonly { op: ConditionOp; label: string; clause: Clause | null }[] = [
    { op: "is_true", label: "est vrai", clause: null },
    { op: "is_false", label: "est faux", clause: null },
    { op: "increased", label: "a augmenté", clause: "post_condition" },
    { op: "decreased", label: "a diminué", clause: "post_condition" },
    { op: "changed", label: "a changé", clause: "post_condition" },
  ];

  readonly canAddCondition = computed(() => this.signals().length > 0);

  add(): void {
    this.intents.update((existing) => [
      ...existing,
      { name: nextName(existing), when: [], click: null, post_condition: [] },
    ]);
  }

  remove(index: number): void {
    this.intents.update((existing) => existing.filter((_, at) => at !== index));
    if (this.picking() === index) {
      this.picking.set(null);
    }
  }

  rename(index: number, name: string): void {
    this.change(index, (intent) => ({ ...intent, name: name.trim().toLowerCase() }));
  }

  pick(index: number): void {
    this.picking.set(this.picking() === index ? null : index);
  }

  addCondition(index: number, clause: Clause): void {
    const signal = this.signals()[0];
    if (signal === undefined) {
      return;
    }
    this.change(index, (intent) => ({
      ...intent,
      [clause]: [...intent[clause], { op: "is_true", signal }],
    }));
  }

  setSignal(index: number, clause: Clause, at: number, signal: string): void {
    this.editCondition(index, clause, at, (condition) => ({ ...condition, signal }));
  }

  opsFor(clause: Clause): readonly { op: ConditionOp; label: string }[] {
    return this.ops.filter((entry) => entry.clause === null || entry.clause === clause);
  }

  setOp(index: number, clause: Clause, at: number, op: string): void {
    const chosen = this.opsFor(clause).find((entry) => entry.op === op);
    if (chosen === undefined) {
      return;
    }
    this.editCondition(index, clause, at, (condition) => ({
      op: chosen.op,
      signal: condition.signal,
    }));
  }

  removeCondition(index: number, clause: Clause, at: number): void {
    this.change(index, (intent) => ({
      ...intent,
      [clause]: intent[clause].filter((_, position) => position !== at),
    }));
  }

  percent(value: number): string {
    return `${Math.round(value * 100)} %`;
  }

  private editCondition(
    index: number,
    clause: Clause,
    at: number,
    edit: (condition: Condition) => Condition,
  ): void {
    this.change(index, (intent) => ({
      ...intent,
      [clause]: intent[clause].map((condition, position) =>
        position === at ? edit(condition) : condition,
      ),
    }));
  }

  private change(index: number, edit: (intent: IntentState) => IntentState): void {
    this.intents.update((existing) =>
      existing.map((intent, at) => (at === index ? edit(intent) : intent)),
    );
  }
}

function nextName(existing: readonly IntentState[]): string {
  let index = existing.length + 1;
  while (existing.some((intent) => intent.name === `action-${index}`)) {
    index += 1;
  }
  return `action-${index}`;
}
