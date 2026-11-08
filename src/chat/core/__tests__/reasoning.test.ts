import { describe, it, expect } from "vitest";
import {
  modeReasoningChoice,
  reasoningChoices,
  resolveReasoningChoice,
  toReasoningLevel,
  REASONING_CHOICES,
  REASONING_DEFAULT,
} from "../reasoning";

const deepMode = { reasoningEnabled: true, reasoningEffort: "high" };
const quickMode = { reasoningEnabled: false, reasoningEffort: null };
const defaultMode = { reasoningEnabled: null, reasoningEffort: null };

describe("toReasoningLevel", () => {
  it("keeps every concrete level as-is", () => {
    for (const level of ["default", "off", "on", "low", "max"]) {
      expect(toReasoningLevel(level)).toBe(level);
    }
  });

  it("folds the internal inherit state into default", () => {
    expect(toReasoningLevel("inherit")).toBe(REASONING_DEFAULT);
  });
});

describe("reasoningChoices", () => {
  it("falls back to the conservative set when levels are missing", () => {
    expect(reasoningChoices(undefined)).toEqual(REASONING_CHOICES);
  });

  it("lists default / off plus every effort when toggle and efforts are declared", () => {
    expect(
      reasoningChoices({ toggle: true, efforts: ["low", "high", "xhigh"] }),
    ).toEqual(["default", "off", "low", "high", "xhigh"]);
  });

  it("offers on when the model only declares a toggle", () => {
    expect(reasoningChoices({ toggle: true, efforts: [] })).toEqual([
      "default",
      "off",
      "on",
    ]);
  });

  it("omits off when the model only declares efforts", () => {
    expect(
      reasoningChoices({ toggle: false, efforts: ["low", "medium"] }),
    ).toEqual(["default", "low", "medium"]);
  });

  it("keeps only default when the model declares neither toggle nor efforts", () => {
    expect(reasoningChoices({ toggle: false, efforts: [] })).toEqual([
      "default",
    ]);
  });

  it("always exposes default first and never inherit", () => {
    expect(reasoningChoices(undefined)[0]).toBe(REASONING_DEFAULT);
    expect(reasoningChoices(undefined)).not.toContain("inherit");
  });
});

describe("modeReasoningChoice", () => {
  it("maps a disabled mode to off", () => {
    expect(modeReasoningChoice(quickMode)).toBe("off");
  });

  it("maps a mode effort to that effort", () => {
    expect(modeReasoningChoice(deepMode)).toBe("high");
  });

  it("maps an enabled mode without effort to on", () => {
    expect(
      modeReasoningChoice({ reasoningEnabled: true, reasoningEffort: null }),
    ).toBe("on");
  });

  it("keeps an effort even when enabled is undeclared", () => {
    expect(
      modeReasoningChoice({ reasoningEnabled: null, reasoningEffort: "low" }),
    ).toBe("low");
  });

  it("returns null when the mode declares nothing", () => {
    expect(modeReasoningChoice(defaultMode)).toBeNull();
    expect(modeReasoningChoice(undefined)).toBeNull();
  });
});

describe("resolveReasoningChoice", () => {
  it("shows the override as-is", () => {
    expect(resolveReasoningChoice("xhigh", quickMode)).toBe("xhigh");
  });

  it("falls back to the mode value when not overridden", () => {
    expect(resolveReasoningChoice("inherit", deepMode)).toBe("high");
  });

  it("falls back to default when neither override nor mode provides a value", () => {
    expect(resolveReasoningChoice("inherit", defaultMode)).toBe(
      REASONING_DEFAULT,
    );
    expect(resolveReasoningChoice("inherit", undefined)).toBe(
      REASONING_DEFAULT,
    );
  });

  it("never returns inherit or null", () => {
    for (const choice of ["inherit", "off", "high", "default"]) {
      const resolved = resolveReasoningChoice(choice, undefined);
      expect(resolved).not.toBe("inherit");
      expect(resolved).not.toBeNull();
    }
  });
});
