import { RuleCondition, SignalValue, Unmet } from "./session.model";

export function showValue(value: SignalValue): string {
  const { type, value: inner } = value;
  if (type === "ratio" && typeof inner === "number") {
    return `${(inner * 100).toFixed(0)} %`;
  }
  if (type === "bool") {
    return inner === true ? "oui" : "non";
  }
  if (typeof inner === "number" || typeof inner === "string") {
    return String(inner);
  }
  return type;
}

function expected(condition: RuleCondition): string {
  switch (condition.op) {
    case "is_true":
      return "doit être vrai";
    case "is_false":
      return "doit être faux";
    case "equals":
      return `doit valoir ${showValue(condition.value)}`;
    case "at_least":
      return `doit valoir au moins ${condition.value}`;
    case "at_most":
      return `doit valoir au plus ${condition.value}`;
    default:
      return "doit varier";
  }
}

export function describe(unmet: Unmet): string {
  const actual =
    unmet.actual === null
      ? "non rapporté par le plugin"
      : `actuel : ${showValue(unmet.actual)}`;
  return `${unmet.condition.signal} ${expected(unmet.condition)} (${actual})`;
}
