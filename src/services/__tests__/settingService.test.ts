import { describe, it, expect, vi, beforeEach } from "vitest";

vi.mock("../system/tauriInvoke", () => ({
  tauriInvoke: vi.fn(),
}));

import { tauriInvoke as invoke } from "../system/tauriInvoke";
import { settingService } from "../settingService";

const mockInvoke = vi.mocked(invoke);

describe("settingService.patchConf", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  it("converts camelCase keys to snake_case before invoking", async () => {
    mockInvoke.mockResolvedValue(undefined);

    await settingService.patchConf("app", {
      stayOnTop: true,
      mainWindowWidth: 800,
      approvalTimeoutSecs: 30,
    });

    expect(mockInvoke).toHaveBeenCalledWith("patch_conf", {
      group: "app",
      patch: {
        stay_on_top: true,
        main_window_width: 800,
        approval_timeout_secs: 30,
      },
    });
  });

  it("keeps already snake_case keys untouched", async () => {
    mockInvoke.mockResolvedValue(undefined);

    await settingService.patchConf("orb", { opacity_percent: 60 });

    expect(mockInvoke).toHaveBeenCalledWith("patch_conf", {
      group: "orb",
      patch: { opacity_percent: 60 },
    });
  });

  it("converts keys one level deep only (nested maps stay verbatim)", async () => {
    mockInvoke.mockResolvedValue(undefined);

    await settingService.patchConf("app", {
      keybindings: { toggleMain: "Ctrl+Shift+Alt+M" },
    });

    expect(mockInvoke).toHaveBeenCalledWith("patch_conf", {
      group: "app",
      patch: { keybindings: { toggleMain: "Ctrl+Shift+Alt+M" } },
    });
  });

  it("passes an empty patch through unchanged", async () => {
    mockInvoke.mockResolvedValue(undefined);

    await settingService.patchConf("app", {});

    expect(mockInvoke).toHaveBeenCalledWith("patch_conf", {
      group: "app",
      patch: {},
    });
  });
});
