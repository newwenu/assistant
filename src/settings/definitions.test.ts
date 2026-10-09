import { describe, it, expect } from "vitest";
import { APP_SETTINGS, ORB_BALL_SETTINGS, ORB_PANEL_SETTINGS, MASCOT_SETTINGS } from "./definitions";

describe("definitions", () => {
  it("APP_SETTINGS covers all expected keys", () => {
    const keys = APP_SETTINGS.map((d) => d.key);
    expect(keys).toContain("language");
    expect(keys).toContain("launch_at_login");
    expect(keys).toContain("close_to_tray");
    expect(keys).toContain("stay_on_top");
  });

  it("APP_SETTINGS all belong to app group", () => {
    expect(APP_SETTINGS.every((d) => d.group === "app")).toBe(true);
  });

  it("ORB_BALL_SETTINGS covers all expected keys", () => {
    const keys = ORB_BALL_SETTINGS.map((d) => d.key);
    expect(keys).toContain("opacity_percent");
    expect(keys).toContain("idle_opacity_percent");
    expect(keys).toContain("scale_percent");
    expect(keys).toContain("click_through");
    expect(keys).toContain("hide_on_pointer_hover");
    expect(keys).toContain("hover_delay_ms");
  });

  it("ORB_BALL_SETTINGS all belong to orb group", () => {
    expect(ORB_BALL_SETTINGS.every((d) => d.group === "orb")).toBe(true);
  });

  it("ORB_PANEL_SETTINGS covers always_on_top", () => {
    const keys = ORB_PANEL_SETTINGS.map((d) => d.key);
    expect(keys).toContain("always_on_top");
    expect(ORB_PANEL_SETTINGS.every((d) => d.group === "orb")).toBe(true);
  });

  it("MASCOT_SETTINGS covers color fields", () => {
    const keys = MASCOT_SETTINGS.map((d) => d.key);
    expect(keys).toContain("primary_color");
    expect(keys).toContain("secondary_color");
    expect(MASCOT_SETTINGS.every((d) => d.group === "mascot")).toBe(true);
  });

  it("slider settings have min/max", () => {
    const sliders = [...ORB_BALL_SETTINGS].filter((d) => d.type === "slider");
    expect(sliders.length).toBeGreaterThan(0);
    expect(sliders.every((d) => d.min !== undefined && d.max !== undefined)).toBe(true);
  });

  it("boolean settings have correct type", () => {
    const booleans = [...APP_SETTINGS, ...ORB_BALL_SETTINGS, ...ORB_PANEL_SETTINGS].filter(
      (d) => d.type === "boolean",
    );
    expect(booleans.length).toBeGreaterThan(0);
    expect(booleans.every((d) => typeof d.default === "boolean")).toBe(true);
  });
});