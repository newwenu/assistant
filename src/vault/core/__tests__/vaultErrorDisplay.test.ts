import { describe, it, expect } from "vitest";
import { translateKey } from "@/i18n";
import { vaultErrorText } from "../display/vaultErrorText";
import { isVaultErrorDto, vaultErrorMessage } from "../display/vaultError";
import type { VaultErrorCode } from "../vault.types";

const NEW_CODES: VaultErrorCode[] = [
  "database",
  "io",
  "clipboard",
  "alreadyUnlocked",
];

describe("vaultErrorText for the four new codes", () => {
  it("maps each new code to its i18n key", () => {
    expect(vaultErrorText("database")).toBe(
      translateKey("vault.error.database"),
    );
    expect(vaultErrorText("io")).toBe(translateKey("vault.error.io"));
    expect(vaultErrorText("clipboard")).toBe(
      translateKey("vault.error.clipboard"),
    );
    expect(vaultErrorText("alreadyUnlocked")).toBe(
      translateKey("vault.error.alreadyUnlocked"),
    );
  });

  it("falls back to the provided message when code is null", () => {
    expect(vaultErrorText(null, "boom")).toBe("boom");
  });
});

describe("isVaultErrorDto recognizes the four new codes", () => {
  it("accepts each new code", () => {
    for (const code of NEW_CODES) {
      expect(isVaultErrorDto({ code, message: "m", retryable: false })).toBe(
        true,
      );
    }
  });

  it("rejects an unknown code", () => {
    expect(
      isVaultErrorDto({ code: "bogus", message: "m", retryable: false }),
    ).toBe(false);
  });

  it("vaultErrorMessage resolves the new codes to localized text", () => {
    expect(
      vaultErrorMessage({ code: "database", message: "raw", retryable: false }),
    ).toBe(translateKey("vault.error.database"));
  });
});
