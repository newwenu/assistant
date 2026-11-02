import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";

const mockInvokeMap = new Map<string, (...args: unknown[]) => unknown>();

vi.mock("@/services/tauriInvoke", () => ({
  tauriInvoke: (cmd: string, args?: unknown): Promise<unknown> => {
    const fn = mockInvokeMap.get(cmd);
    if (fn) return Promise.resolve(fn(args));
    return Promise.resolve(undefined);
  },
}));

import { useVaultStore } from "../vaultStore";
import type { VaultEntryDetail } from "../vault.types";

function mockCmd(cmd: string, impl: (...args: unknown[]) => unknown) {
  mockInvokeMap.set(`plugin:vault|${cmd}`, impl);
}

function vaultError(code: string, message: string): Error {
  return Object.assign(new Error(message), { code, retryable: false });
}

function resetStore() {
  const store = useVaultStore.getState();
  store.clearError();
  useVaultStore.setState({
    isLocked: true,
    isInitialized: false,
    loading: false,
    error: null,
    errorCode: null,
    entries: [],
    selectedEntry: null,
    searchQuery: "",
    categoryFilter: null,
    tagFilter: null,
    revealedField: null,
    revealTimerId: null,
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
    remoteStatus: null,
    remoteChecking: false,
    syncResult: null,
    syncError: null,
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
    mockCmd("vault_unlock", () => Promise.resolve(undefined));

    const store = useVaultStore.getState();
    await store.unlock("correct_password");

    expect(useVaultStore.getState().isLocked).toBe(false);
    expect(useVaultStore.getState().errorCode).toBeNull();
  });

  it("unlock → invalid password stays locked and records code", async () => {
    mockCmd("vault_unlock", () =>
      Promise.reject(vaultError("invalidPassword", "Invalid master password")),
    );

    const store = useVaultStore.getState();
    await store.unlock("wrong_password");

    const state = useVaultStore.getState();
    expect(state.isLocked).toBe(true);
    expect(state.errorCode).toBe("invalidPassword");
    expect(state.error).toBe("Invalid master password");
  });

  it("unlock → invoke throws error stays locked", async () => {
    mockCmd("vault_unlock", () => Promise.reject(new Error("network error")));

    const store = useVaultStore.getState();
    await store.unlock("any");

    const state = useVaultStore.getState();
    expect(state.isLocked).toBe(true);
    expect(state.error).toBeTruthy();
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

  it("createEntry → returns detail and reloads entries", async () => {
    mockCmd("vault_create_entry", () =>
      Promise.resolve({ id: "new-id", title: "GitHub", category: "login" }),
    );
    mockCmd("vault_list_entries", () =>
      Promise.resolve([{ id: "new-id", title: "GitHub", category: "login" }]),
    );

    const store = useVaultStore.getState();
    const detail = await store.createEntry({
      category: "login",
      title: "GitHub",
      username: "octocat",
      tags: [],
      customFields: [],
      isFavorite: false,
    });

    expect(detail.id).toBe("new-id");
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

  it("updateEntry → applies returned detail and reloads entries", async () => {
    useVaultStore.setState({
      selectedEntry: { id: "1", title: "Old", category: "login" } as never,
    });
    mockCmd("vault_update_entry", () =>
      Promise.resolve({ id: "1", title: "Updated", category: "login" }),
    );
    mockCmd("vault_list_entries", () =>
      Promise.resolve([{ id: "1", title: "Updated", category: "login" }]),
    );

    const store = useVaultStore.getState();
    const detail = await store.updateEntry("1", {
      category: "login",
      title: "Updated",
      tags: [],
      customFields: [],
      isFavorite: false,
    });

    expect(detail.title).toBe("Updated");
    expect(useVaultStore.getState().selectedEntry?.title).toBe("Updated");
    expect(useVaultStore.getState().entries[0].title).toBe("Updated");
  });

  it("updateEntry → leaves selection untouched for another entry", async () => {
    useVaultStore.setState({
      selectedEntry: { id: "2", title: "B", category: "login" } as never,
    });
    mockCmd("vault_update_entry", () =>
      Promise.resolve({ id: "1", title: "Updated", category: "login" }),
    );
    mockCmd("vault_list_entries", () => Promise.resolve([]));

    const store = useVaultStore.getState();
    await store.updateEntry("1", {
      category: "login",
      title: "Updated",
      tags: [],
      customFields: [],
      isFavorite: false,
    });

    expect(useVaultStore.getState().selectedEntry?.id).toBe("2");
  });

  it("setFavorite → calls dedicated command and reloads entries", async () => {
    useVaultStore.setState({
      selectedEntry: { id: "1", title: "A", category: "login" } as never,
    });
    const calls: unknown[] = [];
    mockCmd("vault_set_favorite", (args) => {
      calls.push(args);
      return Promise.resolve({
        id: "1",
        title: "A",
        category: "login",
        isFavorite: true,
      });
    });
    mockCmd("vault_list_entries", () =>
      Promise.resolve([
        { id: "1", title: "A", category: "login", isFavorite: true },
      ]),
    );

    const store = useVaultStore.getState();
    await store.setFavorite("1", true);

    expect(calls).toEqual([{ id: "1", isFavorite: true }]);
    expect(useVaultStore.getState().selectedEntry?.isFavorite).toBe(true);
    expect(useVaultStore.getState().entries[0].isFavorite).toBe(true);
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
    const { totpTimerId, revealTimerId } = useVaultStore.getState();
    if (totpTimerId) clearInterval(totpTimerId);
    if (revealTimerId) clearTimeout(revealTimerId);
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
    expect(field!.visible).toBe(true);
  });

  it("revealPassword → reuses cached value within ttl", async () => {
    let calls = 0;
    const expiresAt = Date.now() + 5000;
    mockCmd("vault_reveal_password", () => {
      calls += 1;
      return Promise.resolve({ value: "secret123", expiresAt });
    });

    const store = useVaultStore.getState();
    await store.revealPassword("entry-1");
    store.hideRevealedField();
    expect(useVaultStore.getState().revealedField?.visible).toBe(false);

    const code = await store.revealPassword("entry-1");
    expect(code).toBeNull();
    expect(calls).toBe(1);
    expect(useVaultStore.getState().revealedField?.visible).toBe(true);
  });

  it("revealPassword → refetches when ttl expired", async () => {
    let calls = 0;
    mockCmd("vault_reveal_password", () => {
      calls += 1;
      return Promise.resolve({
        value: `secret-${calls}`,
        expiresAt: Date.now() + 5000,
      });
    });

    const store = useVaultStore.getState();
    await store.revealPassword("entry-1");
    useVaultStore.setState({
      revealedField: {
        entryId: "entry-1",
        value: "stale",
        expiresAt: Date.now() - 1,
        kind: "password",
        visible: false,
      },
    });

    await store.revealPassword("entry-1");
    expect(calls).toBe(2);
    expect(useVaultStore.getState().revealedField?.value).toBe("secret-2");
  });

  it("revealPassword → returns rateLimited code on backend rejection", async () => {
    mockCmd("vault_reveal_password", () =>
      Promise.reject(vaultError("rateLimited", "rate limited")),
    );

    const store = useVaultStore.getState();
    const code = await store.revealPassword("entry-1");

    expect(code).toBe("rateLimited");
    expect(useVaultStore.getState().errorCode).toBe("rateLimited");
    expect(useVaultStore.getState().revealedField).toBeNull();
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

  it("hideRevealedField → hides but keeps cached value", () => {
    useVaultStore.setState({
      revealedField: {
        entryId: "entry-1",
        value: "x",
        expiresAt: Date.now() + 5000,
        kind: "password",
        visible: true,
      },
    });

    useVaultStore.getState().hideRevealedField();

    const field = useVaultStore.getState().revealedField;
    expect(field?.visible).toBe(false);
    expect(field?.value).toBe("x");
  });

  it("clearRevealedField → nulls revealedField and clears timer", () => {
    useVaultStore.setState({
      revealedField: {
        entryId: "entry-1",
        value: "x",
        expiresAt: 0,
        kind: "password",
        visible: true,
      },
    });

    const store = useVaultStore.getState();
    store.clearRevealedField();

    expect(useVaultStore.getState().revealedField).toBeNull();
    expect(useVaultStore.getState().revealTimerId).toBeNull();
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
    mockCmd("vault_unlock_biometric", () => Promise.resolve(undefined));

    const store = useVaultStore.getState();
    await store.unlockBiometric();

    expect(useVaultStore.getState().isLocked).toBe(false);
  });

  it("unlockBiometric → cancelled keeps vault locked and records code", async () => {
    mockCmd("vault_unlock_biometric", () =>
      Promise.reject(vaultError("biometricCancelled", "Biometric cancelled")),
    );

    const store = useVaultStore.getState();
    await store.unlockBiometric();

    const state = useVaultStore.getState();
    expect(state.isLocked).toBe(true);
    expect(state.errorCode).toBe("biometricCancelled");
    expect(state.error).toBe("Biometric cancelled");
  });

  it("unlockBiometric → crypto failure disenrolls credential", async () => {
    useVaultStore.setState({
      biometricEnrolled: true,
      _biometricEnrolledAt: 1,
    });
    mockCmd("vault_unlock_biometric", () =>
      Promise.reject(vaultError("crypto", "Crypto error")),
    );
    mockCmd("vault_disenroll_biometric", () => Promise.resolve(undefined));

    const store = useVaultStore.getState();
    await store.unlockBiometric();

    const state = useVaultStore.getState();
    expect(state.isLocked).toBe(true);
    expect(state.errorCode).toBe("crypto");
    expect(state.biometricEnrolled).toBe(false);
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
    mockCmd("vault_sync", () => Promise.resolve("noRemote"));

    const store = useVaultStore.getState();
    await store.syncPush();

    expect(useVaultStore.getState().syncResult).toBe("noRemote");
    expect(useVaultStore.getState().syncing).toBe(false);
  });

  it("syncNow failure → records syncError and clears syncing", async () => {
    mockCmd("vault_sync", () =>
      Promise.reject(vaultError("gitUnavailable", "git failed")),
    );

    const store = useVaultStore.getState();
    await store.syncNow();

    expect(useVaultStore.getState().syncError).toBe("git failed");
    expect(useVaultStore.getState().errorCode).toBe("gitUnavailable");
    expect(useVaultStore.getState().syncing).toBe(false);
  });

  it("syncPush success → refreshes remote status with fetch=false", async () => {
    const statusCalls: unknown[] = [];
    mockCmd("vault_sync", () => Promise.resolve("success"));
    mockCmd("vault_sync_remote_status", (args) => {
      statusCalls.push(args);
      return Promise.resolve({
        upstream: "origin/main",
        ahead: 0,
        behind: 0,
        fetched: false,
        checkedAt: 1_700_000_000_000,
        errorKind: null,
        errorDetail: null,
      });
    });
    mockCmd("vault_list_entries", () => Promise.resolve([]));
    mockCmd("vault_sync_readyness", () => Promise.resolve("ready"));
    mockCmd("vault_sync_last_time", () => Promise.resolve(null));

    const store = useVaultStore.getState();
    await store.syncPush();

    expect(statusCalls).toEqual([{ fetch: false }]);
    expect(useVaultStore.getState().remoteStatus?.upstream).toBe("origin/main");
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
