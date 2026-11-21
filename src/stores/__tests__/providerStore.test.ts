import { describe, it, expect, beforeEach, vi } from "vitest";

vi.mock("@/i18n", () => ({
  translateKey: (key: string) => key,
}));

vi.mock("../../services/providerService", () => ({
  providerService: {
    listProviders: vi.fn(),
  },
}));

vi.mock("../../settings/core/runtime/settingsDispatcher", () => ({
  dispatchProviderSaved: vi.fn(() => Promise.resolve()),
  dispatchProviderDeleted: vi.fn(() => Promise.resolve()),
}));

vi.mock("../../services/system/logger", () => ({
  createLogger: () => ({
    error: vi.fn(),
    warn: vi.fn(),
    info: vi.fn(),
    debug: vi.fn(),
  }),
}));

import { useProviderStore } from "../providerStore";
import { providerService } from "../../services/providerService";

const PROVIDERS_FAILED = "settings.provider.load_failed";

describe("providerStore load failure visibility", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useProviderStore.setState({
      providers: [],
      loading: false,
      providersError: null,
    });
  });

  it("loadProviders clears providersError on success", async () => {
    vi.mocked(providerService.listProviders).mockResolvedValue([]);
    await useProviderStore.getState().loadProviders();
    expect(useProviderStore.getState().providersError).toBeNull();
  });

  it("loadProviders exposes providersError on failure", async () => {
    vi.mocked(providerService.listProviders).mockRejectedValue(
      new Error("boom"),
    );
    await useProviderStore.getState().loadProviders();
    expect(useProviderStore.getState().providersError).toBe(PROVIDERS_FAILED);
    expect(useProviderStore.getState().loading).toBe(false);
  });
});
