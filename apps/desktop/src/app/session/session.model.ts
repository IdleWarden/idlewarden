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

export interface Refused {
  refusal: { refusal: RefusalKind };
  message: string;
}

/// Mirrors `idlewarden_core::Event`. Hand-copied like the rest of this file,
/// which is what #18 is about.
export type SessionEvent =
  | { event: "game_detected"; plugin: string; window_title: string }
  | { event: "game_lost" }
  | { event: "plugin_loaded"; plugin: string; version: string }
  | { event: "plugin_failed"; plugin: string; reason: string }
  | { event: "observed"; observation: Observation }
  | { event: "intent_proposed"; intent: { name: string } }
  | { event: "intent_rejected"; intent: { name: string }; reason: string }
  | { event: "action_started"; intent: { name: string } }
  | { event: "action_finished"; intent: { name: string }; outcome: unknown }
  | { event: "agent_paused"; reason: string }
  | { event: "agent_resumed" }
  | { event: "kill_switch" }
  | { event: "error"; message: string };

/// `idlewarden_plugin_api::Value`, tagged externally by serde.
export interface SignalValue {
  type: "bool" | "int" | "float" | "ratio" | "text" | "point" | "rect";
  value: unknown;
}

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

export interface PluginSummary {
  id: string;
  detected: boolean;
  intents: IntentSummary[];
}
