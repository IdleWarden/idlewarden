export type UpdateChannel = "stable" | "beta";

export interface UpdateSettings {
  channel: UpdateChannel;
}

export interface UpdateOffer {
  version: string;
  pub_date: string;
  url: string;
  notes: string | null;
}

export type CheckResult =
  { outcome: "up_to_date" } | { outcome: "available"; offer: UpdateOffer };
