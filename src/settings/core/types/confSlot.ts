import type { SettingsPageId } from "./pageTypes";
import type { ConfGroup } from "./confRegistry";

export interface ConfSlot<G extends ConfGroup = ConfGroup> {
  group: G;
  invoke: string;
  namespace?: string;
  pageId?: SettingsPageId;
}
