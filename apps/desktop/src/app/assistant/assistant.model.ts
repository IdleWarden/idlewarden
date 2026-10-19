export type AssistantReply =
  | { kind: "partial"; text: string }
  | { kind: "done"; text: string }
  | { kind: "failed"; message: string };

export interface ChatMessage {
  from: "player" | "warden";
  text: string;
  pending: boolean;
  failed: boolean;
}
