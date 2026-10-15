export interface AppConf {
  theme: string;
  language: string;
  hotkey: string;
  stay_on_top: boolean;
  launch_at_login: boolean;
  close_to_tray: boolean;
  keybindings: Record<string, string>;
  auto_compact: boolean;
  compact_threshold_percent: number;
  compact_summary_prompt: string | null;
  compact_summary_max_tokens: number;
}

export interface OrbConf {
  click_through: boolean;
  always_on_top: boolean;
  scale_percent: number;
  opacity_percent: number;
  hide_on_pointer_hover: boolean;
  hover_delay_ms: number;
  idle_opacity_percent: number;
  position_x: number | null;
  position_y: number | null;
}

export interface MascotConf {
  renderer: string;
  primary_color: string | null;
  secondary_color: string | null;
  rive_idle_pose_rotation: boolean;
  rive_src: string | null;
  svg_animated: boolean;
  gif_src: string | null;
  gif_border_radius: number;
}
