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
