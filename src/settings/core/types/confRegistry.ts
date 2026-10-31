import type { AppConf, OrbConf, MascotConf } from "./confTypes";

export interface ConfTypeRegistry {
  app: AppConf;
  orb: OrbConf;
  mascot: MascotConf;
}

export type ConfGroup = Extract<keyof ConfTypeRegistry, string>;

export type ConfOf<G extends ConfGroup> = ConfTypeRegistry[G];

export type ConfStore = {
  [G in ConfGroup]?: ConfOf<G> | null;
};
