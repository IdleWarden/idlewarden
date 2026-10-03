export type Corner = "top_left" | "top_right" | "bottom_left" | "bottom_right";

export interface Hotkeys {
  toggle_watch: string;
  toggle_panel: string;
  toggle_overlay: string;
  kill_switch: string;
}

export interface OverlaySettings {
  shown: boolean;
  corner: Corner;
  margin_x: number;
  margin_y: number;
  hotkeys: Hotkeys;
}
