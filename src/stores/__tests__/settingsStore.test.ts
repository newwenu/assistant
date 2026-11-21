import { describe, it, expect, beforeEach, vi } from "vitest";

vi.mock("@/i18n", () => ({
  translateKey: (key: string) => key,
}));

vi.mock("@/i18n/localeApplier", () => ({
  applyLocaleChange: vi.fn(() => Promise.resolve()),
}));

vi.mock("../../settings/core/runtime/confRuntime", () => ({
  getConfSlot: vi.fn(() => ({ group: "app", namespace: null })),
  fetchConf: vi.fn(),
}));

import { useSettingsStore } from "../settingsStore";
import { fetchConf } from "../../settings/core/runtime/confRuntime";

const CONF_FAILED = "settings.load_conf_failed";

describe("settingsStore load failure visibility", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useSettingsStore.setState({ confs: {}, confErrors: {} });
  });

  it("loadConf clears the group error on success", async () => {
    vi.mocked(fetchConf).mockResolvedValue(null);
    await useSettingsStore.getState().loadConf("app");
    expect(useSettingsStore.getState().confErrors.app).toBeUndefined();
  });

  it("loadConf exposes the group error on failure", async () => {
    vi.mocked(fetchConf).mockRejectedValue(new Error("boom"));
    await useSettingsStore.getState().loadConf("app");
    expect(useSettingsStore.getState().confErrors.app).toBe(CONF_FAILED);
  });

  it("a later successful load clears the previous error", async () => {
    vi.mocked(fetchConf).mockRejectedValueOnce(new Error("boom"));
    await useSettingsStore.getState().loadConf("app");
    expect(useSettingsStore.getState().confErrors.app).toBe(CONF_FAILED);

    vi.mocked(fetchConf).mockResolvedValueOnce(null);
    await useSettingsStore.getState().loadConf("app");
    expect(useSettingsStore.getState().confErrors.app).toBeUndefined();
  });
});
