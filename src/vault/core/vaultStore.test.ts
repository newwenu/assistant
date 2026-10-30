import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";

const mockInvokeMap = new Map<string, (...args: unknown[]) => unknown>();

vi.mock("@/services/tauriInvoke", () => ({
  tauriInvoke: (cmd: string, args?: unknown): Promise<unknown> => {
    const fn = mockInvokeMap.get(cmd);
    if (fn) return Promise.resolve(fn(args));
    return Promise.resolve(undefined);
  },
}));

import { useVaultStore } from "./vaultStore";
import type { VaultEntryDetail } from "./vault.types";

function mockCmd(cmd: string, impl: (...args: unknown[]) => unknown) {
  mockInvokeMap.set(cmd, impl);
}

function resetStore() {
  const store = useVaultStore.getState();
  store.clearError();
  useVaultStore.setState({
    isLocked: true,
    isInitialized: false,
    loading: false,
    error: null,
    entries: [],
    selectedEntry: null,
    searchQuery: "",
    categoryFilter: null,
    tagFilter: null,
    revealedField: null,
    totpCode: null,
    totpTimeRemaining: 0,
    hibpLeaks: {},
    hibpAuditing: false,
    biometricAvailable: false,
    biometricEnrolled: false,
    _biometricAvailableAt: 0,
    _biometricEnrolledAt: 0,
    _biometricRetryTimerId: null,
    _biometricAvailRetryCount: 0,
    _biometricEnrolledRetryCount: 0,
    _recoveryKeySetAt: 0,
    syncReadyness: null,
    syncResult: null,
    syncConfig: null,
    syncing: false,
    sshAgentKeys: [],
    sshAgentRunning: false,
    recoveryKeySet: false,
    integrityResult: null,
  });
}

function clearMocks() {
  mockInvokeMap.clear();
}

describe("vaultStore lifecycle", () => {
  beforeEach(() => {
    clearMocks();
    resetStore();
  });

  afterEach(() => {
    const { autoLockTimerId, totpTimerId } = useVaultStore.getState();
    if (autoLockTimerId) clearTimeout(autoLockTimerId);
    if (totpTimerId) clearInterval(totpTimerId);
  });

  it("init → unlocked state", async () => {
    mockCmd("vault_init", () => Promise.resolve(undefined));

    const store = useVaultStore.getState();
    await store.init("Str0ng#P@ssw0rd!2025");

    const state = useVaultStore.getState();
    expect(state.isLocked).toBe(false);
    expect(state.isInitialized).toBe(true);
    expect(state.loading).toBe(false);
    expect(state.error).toBeNull();
  });

  it("init failure → error set, still locked", async () => {
    mockCmd("vault_init", () => Promise.reject(new Error("weak password")));

    const store = useVaultStore.getState();
    await store.init("123");

    const state = useVaultStore.getState();
    expect(state.isLocked).toBe(true);
    expect(state.error).toBeTruthy();
  });

  it("unlock → correct password unlocks", async () => {
    mockCmd("vault_unlock", () => Promise.resolve(true));

    const store = useVaultStore.getState();
    const ok = await store.unlock("correct_password");

    expect(ok).toBe(true);
    expect(useVaultStore.getState().isLocked).toBe(false);
  });

  it("unlock → wrong password stays locked", async () => {
    mockCmd("vault_unlock", () => Promise.resolve(false));

    const store = useVaultStore.getState();
    const ok = await store.unlock("wrong_password");

    expect(ok).toBe(false);
    expect(useVaultStore.getState().isLocked).toBe(true);
    expect(useVaultStore.getState().error).toBe("Invalid password");
  });

  it("unlock → invoke throws error stays locked", async () => {
    mockCmd("vault_unlock", () => Promise.reject(new Error("network error")));

    const store = useVaultStore.getState();
    const ok = await store.unlock("any");

    expect(ok).toBe(false);
    expect(useVaultStore.getState().isLocked).toBe(true);
    expect(useVaultStore.getState().error).toBeTruthy();
  });

  it("lock → clears entries and selectedEntry", async () => {
    mockCmd("vault_lock", () => Promise.resolve(undefined));

    useVaultStore.setState({
      isLocked: false,
      entries: [
        {
          id: "1",
          title: "Test",
          category: "login",
          username: null,
          url: null,
          tags: [],
          isFavorite: false,
          updatedAt: 0,
        },
      ],
      selectedEntry: {
        id: "1",
        title: "Test",
        category: "login",
        username: null,
        url: null,
        tags: [],
        isFavorite: false,
        updatedAt: 0,
      } as unknown as VaultEntryDetail,
    });

    const store = useVaultStore.getState();
    await store.lock();

    const state = useVaultStore.getState();
    expect(state.isLocked).toBe(true);
    expect(state.entries).toEqual([]);
    expect(state.selectedEntry).toBeNull();
    expect(state.revealedField).toBeNull();
  });

  it("checkStatus → sets isLocked and isInitialized", async () => {
    mockCmd("vault_is_locked", () => Promise.resolve(false));
    mockCmd("vault_is_initialized", () => Promise.resolve(true));

    const store = useVaultStore.getState();
    await store.checkStatus();

    const state = useVaultStore.getState();
    expect(state.isLocked).toBe(false);
    expect(state.isInitialized).toBe(true);
  });
});

describe("vaultStore CRUD", () => {
  beforeEach(() => {
    clearMocks();
    resetStore();
    useVaultStore.setState({ isLocked: false });
  });

  afterEach(() => {
    const { autoLockTimerId } = useVaultStore.getState();
    if (autoLockTimerId) clearTimeout(autoLockTimerId);
  });

  it("createEntry → reloads entries after creation", async () => {
    mockCmd("vault_create_entry", () => Promise.resolve("new-id"));
    mockCmd("vault_list_entries", () =>
      Promise.resolve([{ id: "new-id", title: "GitHub", category: "login" }]),
    );

    const store = useVaultStore.getState();
    const id = await store.createEntry({
      category: "login",
      title: "GitHub",
      username: "octocat",
      tags: [],
      customFields: [],
      isFavorite: false,
    });

    expect(id).toBe("new-id");
    expect(useVaultStore.getState().entries).toHaveLength(1);
  });

  it("deleteEntry → removes from list and clears selection if same", async () => {
    useVaultStore.setState({
      entries: [
        {
          id: "1",
          title: "Test",
          category: "login",
          username: null,
          url: null,
          tags: [],
          isFavorite: false,
          updatedAt: 0,
        },
      ],
      selectedEntry: {
        id: "1",
        title: "Test",
        category: "login",
        username: null,
        url: null,
        tags: [],
        isFavorite: false,
        updatedAt: 0,
      } as unknown as VaultEntryDetail,
    });

    mockCmd("vault_delete_entry", () => Promise.resolve(undefined));
    mockCmd("vault_list_entries", () => Promise.resolve([]));

    const store = useVaultStore.getState();
    await store.deleteEntry("1");

    const state = useVaultStore.getState();
    expect(state.selectedEntry).toBeNull();
    expect(state.entries).toHaveLength(0);
  });

  it("deleteEntry → keeps selection if different entry", async () => {
    useVaultStore.setState({
      entries: [
        {
          id: "1",
          title: "A",
          category: "login",
          username: null,
          url: null,
          tags: [],
          isFavorite: false,
          updatedAt: 0,
        },
        {
          id: "2",
          title: "B",
          category: "login",
          username: null,
          url: null,
          tags: [],
          isFavorite: false,
          updatedAt: 0,
        },
      ],
      selectedEntry: {
        id: "2",
        title: "B",
        category: "login",
        username: null,
        url: null,
        tags: [],
        isFavorite: false,
        updatedAt: 0,
      } as unknown as VaultEntryDetail,
    });

    mockCmd("vault_delete_entry", () => Promise.resolve(undefined));
    mockCmd("vault_list_entries", () =>
      Promise.resolve([{ id: "2", title: "B" }]),
    );

    const store = useVaultStore.getState();
    await store.deleteEntry("1");

    expect(useVaultStore.getState().selectedEntry).not.toBeNull();
  });

  it("updateEntry → reloads entries after update", async () => {
    mockCmd("vault_update_entry", () => Promise.resolve(undefined));
    mockCmd("vault_list_entries", () =>
      Promise.resolve([{ id: "1", title: "Updated", category: "login" }]),
    );

    const store = useVaultStore.getState();
    await store.updateEntry("1", {
      category: "login",
      title: "Updated",
      tags: [],
      customFields: [],
      isFavorite: false,
    });

    expect(useVaultStore.getState().entries[0].title).toBe("Updated");
  });
});

describe("vaultStore search & filter", () => {
  beforeEach(() => {
    clearMocks();
    resetStore();
  });

  it("setSearchQuery updates state", () => {
    const store = useVaultStore.getState();
    store.setSearchQuery("Git");
    expect(useVaultStore.getState().searchQuery).toBe("Git");
  });

  it("setCategoryFilter updates state", () => {
    const store = useVaultStore.getState();
    store.setCategoryFilter("login");
    expect(useVaultStore.getState().categoryFilter).toBe("login");
  });

  it("setTagFilter updates state", () => {
    const store = useVaultStore.getState();
    store.setTagFilter("work");
    expect(useVaultStore.getState().tagFilter).toBe("work");
  });
});

describe("vaultStore reveal & TOTP", () => {
  beforeEach(() => {
    clearMocks();
    resetStore();
  });

  afterEach(() => {
    const { totpTimerId } = useVaultStore.getState();
    if (totpTimerId) clearInterval(totpTimerId);
  });

  it("revealPassword → sets revealedField with expiry", async () => {
    const expiresAt = Date.now() + 5000;
    mockCmd("vault_reveal_password", () =>
      Promise.resolve({ value: "secret123", expiresAt }),
    );

    const store = useVaultStore.getState();
    await store.revealPassword("entry-1");

    const field = useVaultStore.getState().revealedField;
    expect(field).not.toBeNull();
    expect(field!.value).toBe("secret123");
    expect(field!.kind).toBe("password");
    expect(field!.expiresAt).toBe(expiresAt);
  });

  it("revealNotes → sets revealedField with kind=notes", async () => {
    const expiresAt = Date.now() + 5000;
    mockCmd("vault_reveal_notes", () =>
      Promise.resolve({ value: "my notes", expiresAt }),
    );

    const store = useVaultStore.getState();
    await store.revealNotes("entry-1");

    const field = useVaultStore.getState().revealedField;
    expect(field).not.toBeNull();
    expect(field!.kind).toBe("notes");
  });

  it("clearRevealedField → nulls revealedField", () => {
    useVaultStore.setState({
      revealedField: { value: "x", expiresAt: 0, kind: "password" },
    });

    const store = useVaultStore.getState();
    store.clearRevealedField();

    expect(useVaultStore.getState().revealedField).toBeNull();
  });

  it("generateTotp → sets totpCode and totpTimeRemaining", async () => {
    mockCmd("vault_totp_generate", () =>
      Promise.resolve({ code: "123456", timeRemaining: 25 }),
    );

    const store = useVaultStore.getState();
    await store.generateTotp("entry-1");

    const state = useVaultStore.getState();
    expect(state.totpCode).toBe("123456");
    expect(state.totpTimeRemaining).toBe(25);
  });

  it("stopTotpTimer → clears TOTP state", () => {
    useVaultStore.setState({
      totpCode: "123456",
      totpTimeRemaining: 15,
      totpTimerId: setInterval(() => {}, 99999),
    });

    const store = useVaultStore.getState();
    store.stopTotpTimer();

    const state = useVaultStore.getState();
    expect(state.totpCode).toBeNull();
    expect(state.totpTimeRemaining).toBe(0);
    expect(state.totpTimerId).toBeNull();
  });
});

describe("vaultStore HIBP", () => {
  beforeEach(() => {
    clearMocks();
    resetStore();
  });

  it("hibpCheck → stores leak count", async () => {
    mockCmd("vault_hibp_check", () => Promise.resolve(42));

    const store = useVaultStore.getState();
    const count = await store.hibpCheck("entry-1");

    expect(count).toBe(42);
    expect(useVaultStore.getState().hibpLeaks["entry-1"]).toBe(42);
  });

  it("hibpCheck → returns 0 on error", async () => {
    mockCmd("vault_hibp_check", () => Promise.reject(new Error("network")));

    const store = useVaultStore.getState();
    const count = await store.hibpCheck("entry-1");

    expect(count).toBe(0);
  });
});

describe("vaultStore biometric", () => {
  beforeEach(() => {
    clearMocks();
    resetStore();
  });

  afterEach(() => {
    const { autoLockTimerId, _biometricRetryTimerId } =
      useVaultStore.getState();
    if (autoLockTimerId) clearTimeout(autoLockTimerId);
    if (_biometricRetryTimerId) clearTimeout(_biometricRetryTimerId);
  });

  it("checkBiometricAvailable → sets biometricAvailable", async () => {
    mockCmd("vault_is_biometric_available", () => Promise.resolve(true));

    const store = useVaultStore.getState();
    await store.checkBiometricAvailable();

    expect(useVaultStore.getState().biometricAvailable).toBe(true);
  });

  it("checkBiometricAvailable → failure does not stamp cache", async () => {
    mockCmd("vault_is_biometric_available", () =>
      Promise.reject(new Error("keyring unavailable")),
    );

    const store = useVaultStore.getState();
    await store.checkBiometricAvailable();

    expect(useVaultStore.getState().biometricAvailable).toBe(false);
    expect(useVaultStore.getState()._biometricAvailableAt).toBe(0);
  });

  it("checkBiometricEnrolled → failure does not stamp cache", async () => {
    mockCmd("vault_is_biometric_enrolled", () =>
      Promise.reject(new Error("SecurityDbState not managed")),
    );

    const store = useVaultStore.getState();
    await store.checkBiometricEnrolled();

    expect(useVaultStore.getState().biometricEnrolled).toBe(false);
    expect(useVaultStore.getState()._biometricEnrolledAt).toBe(0);
  });

  it("checkBiometricAvailable → failure schedules retry, enrolled success keeps own counter", async () => {
    mockCmd("vault_is_biometric_available", () =>
      Promise.reject(new Error("keyring unavailable")),
    );
    mockCmd("vault_is_biometric_enrolled", () => Promise.resolve(true));

    const store = useVaultStore.getState();
    await store.checkBiometricAvailable();
    await store.checkBiometricEnrolled();

    const state = useVaultStore.getState();
    expect(state._biometricAvailRetryCount).toBe(1);
    expect(state._biometricEnrolledRetryCount).toBe(0);
    expect(state._biometricRetryTimerId).not.toBeNull();
  });

  it("retry uses exponential backoff starting at 250ms", async () => {
    vi.useFakeTimers();
    try {
      let calls = 0;
      mockCmd("vault_is_biometric_available", () => {
        calls += 1;
        return Promise.reject(new Error("keyring unavailable"));
      });
      mockCmd("vault_is_biometric_enrolled", () => Promise.resolve(true));

      const store = useVaultStore.getState();
      await store.checkBiometricAvailable();
      expect(calls).toBe(1);

      await vi.advanceTimersByTimeAsync(249);
      expect(calls).toBe(1);

      await vi.advanceTimersByTimeAsync(1);
      expect(calls).toBe(2);
    } finally {
      vi.useRealTimers();
    }
  });

  it("unlockBiometric → success unlocks vault", async () => {
    mockCmd("vault_unlock_biometric", () => Promise.resolve(true));

    const store = useVaultStore.getState();
    const ok = await store.unlockBiometric();

    expect(ok).toBe(true);
    expect(useVaultStore.getState().isLocked).toBe(false);
  });

  it("unlockBiometric → failure keeps vault locked", async () => {
    mockCmd("vault_unlock_biometric", () => Promise.resolve(false));

    const store = useVaultStore.getState();
    const ok = await store.unlockBiometric();

    expect(ok).toBe(false);
    expect(useVaultStore.getState().isLocked).toBe(true);
  });
});

describe("vaultStore recovery key", () => {
  beforeEach(() => {
    clearMocks();
    resetStore();
  });

  afterEach(() => {
    const { autoLockTimerId } = useVaultStore.getState();
    if (autoLockTimerId) clearTimeout(autoLockTimerId);
  });

  it("generateRecoveryKey → returns key and sets recoveryKeySet", async () => {
    mockCmd("vault_generate_recovery_key", () =>
      Promise.resolve("ABCDEFGH-IJKLMNOP-QRSTUVWX-YZ234567"),
    );

    const store = useVaultStore.getState();
    const key = await store.generateRecoveryKey();

    expect(key).toBe("ABCDEFGH-IJKLMNOP-QRSTUVWX-YZ234567");
    expect(useVaultStore.getState().recoveryKeySet).toBe(true);
  });
});

describe("vaultStore sync", () => {
  beforeEach(() => {
    clearMocks();
    resetStore();
  });

  it("syncPull → sets syncResult", async () => {
    mockCmd("vault_sync", () => Promise.resolve("success"));

    const store = useVaultStore.getState();
    await store.syncPull();

    expect(useVaultStore.getState().syncResult).toBe("success");
    expect(useVaultStore.getState().syncing).toBe(false);
  });

  it("syncPush → sets syncResult", async () => {
    mockCmd("vault_sync", () => Promise.resolve("upToDate"));

    const store = useVaultStore.getState();
    await store.syncPush();

    expect(useVaultStore.getState().syncResult).toBe("upToDate");
    expect(useVaultStore.getState().syncing).toBe(false);
  });

  it("loadSyncConfig → sets syncConfig", async () => {
    const config = {
      remoteUrl: "git@github.com:user/vault.git",
      remoteName: "origin",
      sshKeyPath: null,
    };
    mockCmd("vault_get_sync_config", () => Promise.resolve(config));

    const store = useVaultStore.getState();
    await store.loadSyncConfig();

    expect(useVaultStore.getState().syncConfig).toEqual(config);
  });
});