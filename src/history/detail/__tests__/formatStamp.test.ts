import { describe, it, expect } from "vitest";
import { formatStamp } from "../formatStamp";

describe("formatStamp", () => {
  it("pads short mode to HH:MM:SS", () => {
    const stamp = new Date(2024, 0, 2, 3, 4, 5).getTime();
    expect(formatStamp(stamp)).toBe("03:04:05");
  });

  it("prepends the date in full mode", () => {
    const stamp = new Date(2024, 0, 2, 3, 4, 5).getTime();
    expect(formatStamp(stamp, true)).toBe("2024-01-02 03:04:05");
  });
});
