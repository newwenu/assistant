import { describe, it, expect } from "vitest";
import { deriveSyncDisplay, redactUrlUserInfo } from "../display/syncDisplay";
import type { RemoteStatus } from "../vault.types";

function remoteStatus(overrides: Partial<RemoteStatus> = {}): RemoteStatus {
  return {
    upstream: "origin/main",
    ahead: null,
    behind: null,
    fetched: false,
    checkedAt: 1_700_000_000_000,
    errorKind: null,
    errorDetail: null,
    ...overrides,
  };
}

describe("deriveSyncDisplay", () => {
  it("repository state wins over syncError", () => {
    expect(
      deriveSyncDisplay({
        readyness: { repoState: "merge" },
        remoteStatus: null,
        syncError: "boom",
      }),
    ).toEqual({ kind: "repoState", state: "merge" });
  });

  it("syncError wins over plain readyness", () => {
    expect(
      deriveSyncDisplay({
        readyness: "noRemote",
        remoteStatus: null,
        syncError: "boom",
      }),
    ).toEqual({ kind: "failed", message: "boom" });
  });

  it("null readyness is unknown", () => {
    expect(
      deriveSyncDisplay({
        readyness: null,
        remoteStatus: null,
        syncError: null,
      }),
    ).toEqual({ kind: "unknown" });
  });

  it("repository state maps to repoState", () => {
    expect(
      deriveSyncDisplay({
        readyness: { repoState: "merge" },
        remoteStatus: null,
        syncError: null,
      }),
    ).toEqual({ kind: "repoState", state: "merge" });
  });

  it("maps noSync", () => {
    expect(
      deriveSyncDisplay({
        readyness: "noSync",
        remoteStatus: null,
        syncError: null,
      }),
    ).toEqual({ kind: "noSync" });
  });

  it("noRemote wins over dirty", () => {
    expect(
      deriveSyncDisplay({
        readyness: "noRemote",
        remoteStatus: remoteStatus({ ahead: 2, behind: 0 }),
        syncError: null,
      }),
    ).toEqual({ kind: "noRemote" });
  });

  it("maps dirty", () => {
    expect(
      deriveSyncDisplay({
        readyness: "dirty",
        remoteStatus: null,
        syncError: null,
      }),
    ).toEqual({ kind: "dirty" });
  });

  it("ready without remote status is readyUnchecked(notChecked)", () => {
    expect(
      deriveSyncDisplay({
        readyness: "ready",
        remoteStatus: null,
        syncError: null,
      }),
    ).toEqual({ kind: "readyUnchecked", reason: "notChecked" });
  });

  it("ready with zero counts is inSync", () => {
    expect(
      deriveSyncDisplay({
        readyness: "ready",
        remoteStatus: remoteStatus({
          ahead: 0,
          behind: 0,
          fetched: true,
          checkedAt: 1_700_000_000_000,
        }),
        syncError: null,
      }),
    ).toEqual({
      kind: "inSync",
      checkedAt: 1_700_000_000_000,
      fetched: true,
    });
  });

  it("ready with ahead count is ahead", () => {
    expect(
      deriveSyncDisplay({
        readyness: "ready",
        remoteStatus: remoteStatus({ ahead: 2, behind: 0 }),
        syncError: null,
      }),
    ).toEqual({ kind: "ahead", ahead: 2 });
  });

  it("ready with behind count is behind", () => {
    expect(
      deriveSyncDisplay({
        readyness: "ready",
        remoteStatus: remoteStatus({ ahead: 0, behind: 3 }),
        syncError: null,
      }),
    ).toEqual({ kind: "behind", behind: 3 });
  });

  it("ready with both counts is diverged", () => {
    expect(
      deriveSyncDisplay({
        readyness: "ready",
        remoteStatus: remoteStatus({ ahead: 1, behind: 1 }),
        syncError: null,
      }),
    ).toEqual({ kind: "diverged", ahead: 1, behind: 1 });
  });

  it("unknown counts with an error is readyUnchecked(checkFailed)", () => {
    expect(
      deriveSyncDisplay({
        readyness: "ready",
        remoteStatus: remoteStatus({
          errorKind: "noUpstream",
          errorDetail: "no upstream",
        }),
        syncError: null,
      }),
    ).toEqual({ kind: "readyUnchecked", reason: "checkFailed" });
  });

  it("never claims inSync when the remote check failed", () => {
    expect(
      deriveSyncDisplay({
        readyness: "ready",
        remoteStatus: remoteStatus({
          ahead: 0,
          behind: 0,
          errorKind: "fetchFailed",
          errorDetail: "network down",
        }),
        syncError: null,
      }),
    ).toEqual({ kind: "readyUnchecked", reason: "checkFailed" });
  });

  it("keeps counts when fetch failed but refs are usable", () => {
    expect(
      deriveSyncDisplay({
        readyness: "ready",
        remoteStatus: remoteStatus({
          ahead: 1,
          behind: 0,
          errorKind: "fetchFailed",
          errorDetail: "network down",
        }),
        syncError: null,
      }),
    ).toEqual({ kind: "ahead", ahead: 1 });
  });
});

describe("T-S12 remote URL redaction", () => {
  it("strips userinfo from a full URL", () => {
    expect(
      redactUrlUserInfo("https://alice:secret@git.example.com/repo.git"),
    ).toBe("https://git.example.com/repo.git");
  });

  it("strips userinfo embedded in an error message", () => {
    const message =
      "fatal: unable to access 'https://token@git.example.com/repo.git/': 403";
    const redacted = redactUrlUserInfo(message);
    expect(redacted).not.toContain("token");
    expect(redacted).toContain("https://git.example.com/repo.git/");
  });

  it("leaves URLs without userinfo untouched", () => {
    expect(redactUrlUserInfo("https://git.example.com/repo.git")).toBe(
      "https://git.example.com/repo.git",
    );
  });

  it("failed sync display never shows credentials", () => {
    const display = deriveSyncDisplay({
      readyness: null,
      remoteStatus: null,
      syncError:
        "git clone failed: https://alice:secret@git.example.com/repo.git",
    });
    expect(display.kind).toBe("failed");
    expect(display.kind === "failed" && display.message).not.toContain(
      "secret",
    );
  });
});
