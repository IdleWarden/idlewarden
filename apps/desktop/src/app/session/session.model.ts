export type SessionState = "searching" | "ready" | "running" | "paused" | "halted";

export interface Session {
  state: SessionState;
  dry_run: boolean;
  actions_taken: number;
  last_reason: string | null;
  plugin: string | null;
  profile: string | null;
}

export type Command =
  | { command: "start"; plugin: string; profile: string }
  | { command: "stop" }
  | { command: "pause" }
  | { command: "resume" }
  | { command: "set_dry_run"; enabled: boolean };

export type RefusalKind =
  "no_game_ready" | "halted" | "not_running" | "not_paused" | "running_dry_run_change";

export interface Refusal {
  refusal: RefusalKind;
}

export interface Refused {
  refusal: Refusal;
  message: string;
}

export type SignalValue =
  | { type: "bool"; value: boolean }
  | { type: "int"; value: number }
  | { type: "float"; value: number }
  | { type: "ratio"; value: number }
  | { type: "text"; value: string }
  | { type: "point"; value: { x: number; y: number } }
  | { type: "rect"; value: { x: number; y: number; w: number; h: number } }
  | { type: "enum"; value: string };

export interface Intent {
  name: string;
  params: Record<string, SignalValue>;
}

export type ActionOutcome =
  | { outcome: "succeeded" }
  | { outcome: "failed"; reason: string }
  | { outcome: "rejected"; reason: string }
  | { outcome: "aborted" }
  | { outcome: "timed_out"; after_ms: number };

export type SessionEvent =
  | { event: "game_detected"; plugin: string; window_title: string }
  | { event: "game_lost" }
  | { event: "plugin_loaded"; plugin: string; version: string }
  | { event: "plugin_failed"; plugin: string; reason: string }
  | { event: "observed"; observation: Observation }
  | { event: "intent_proposed"; intent: Intent }
  | { event: "intent_rejected"; intent: Intent; reason: string }
  | { event: "action_started"; intent: Intent }
  | { event: "action_finished"; intent: Intent; outcome: ActionOutcome }
  | { event: "agent_paused"; reason: string }
  | { event: "agent_resumed" }
  | { event: "kill_switch" }
  | { event: "error"; message: string };

/// A `SessionEvent` as the desktop hands it over: the Core's own payload plus
/// the wall-clock moment the app observed it.
export type PublishedEvent = SessionEvent & { at_ms: number };

export interface Signal {
  id: string;
  value: SignalValue;
  confidence: number;
}

export interface Observation {
  frame_id: number;
  captured_at_ms: number;
  signals: Signal[];
}

export interface IntentSummary {
  name: string;
  enabled: boolean;
}

export type LogLevel = "ERROR" | "WARN" | "INFO" | "DEBUG" | "TRACE";

/// One `tracing` event. `fields` keeps the structured values the Core emitted
/// rather than a rendered line, which is what makes them filterable.
export interface LogRecord {
  at_ms: number;
  level: LogLevel;
  target: string;
  message: string;
  fields: Record<string, unknown>;
}

export interface WindowCandidate {
  title: string;
  executable: string;
  steam_appid: number | null;
  plugins: string[];
}

export interface PluginSummary {
  id: string;
  detected: boolean;
  intents: IntentSummary[];
}
