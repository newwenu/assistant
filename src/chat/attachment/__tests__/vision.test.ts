import { describe, expect, it } from "vitest";
import type { Provider } from "@/settings/provider/types";
import { resolveVisionSupport } from "../vision";

function model(overrides: Partial<Provider["models"][number]> = {}) {
  return {
    id: "m1",
    name: "M1",
    ownedBy: "test",
    contextWindow: null,
    supportVision: false,
    inputModalities: [] as string[],
    reasoningLevels: null,
    vision: null,
    pdf: null,
    ...overrides,
  };
}

function provider(models: ReturnType<typeof model>[]): Provider {
  return {
    id: "p1",
    name: "P1",
    defaultModel: "m1",
    models,
  } as Provider;
}

describe("resolveVisionSupport", () => {
  it("returns unsupported when provider or model is missing", () => {
    expect(resolveVisionSupport([], "p1", "m1")).toBe("unsupported");
    expect(resolveVisionSupport([provider([model()])], "p1", "nope")).toBe(
      "unsupported",
    );
    expect(resolveVisionSupport([provider([model()])], null, "m1")).toBe(
      "unsupported",
    );
  });

  it("prefers the vision capability fact over legacy fields", () => {
    const p = provider([
      model({
        supportVision: false,
        inputModalities: [],
        vision: { support: "supported", source: "catalog" },
      }),
    ]);
    expect(resolveVisionSupport([p], "p1", "m1")).toBe("supported");
  });

  it("derives from inputModalities when vision fact is absent", () => {
    const withImage = provider([model({ inputModalities: ["text", "image"] })]);
    const textOnly = provider([model({ inputModalities: ["text"] })]);
    expect(resolveVisionSupport([withImage], "p1", "m1")).toBe("supported");
    expect(resolveVisionSupport([textOnly], "p1", "m1")).toBe("unsupported");
  });

  it("falls back to legacy supportVision when modalities are empty", () => {
    const legacyVision = provider([model({ supportVision: true })]);
    expect(resolveVisionSupport([legacyVision], "p1", "m1")).toBe("supported");
    expect(resolveVisionSupport([provider([model()])], "p1", "m1")).toBe(
      "unsupported",
    );
  });
});
