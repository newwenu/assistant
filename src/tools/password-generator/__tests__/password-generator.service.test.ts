import { describe, it, expect } from "vitest";
import {
  SPECIAL,
  CHARSET_SIZES,
  getCharsetLength,
  getCrackTimeEstimation,
  formatDuration,
  getStrengthLevel,
  generatePassword,
} from "../password-generator.service";

describe("getCharsetLength", () => {
  it("returns 0 for empty string", () => {
    expect(getCharsetLength("")).toBe(0);
  });

  it("detects lowercase only", () => {
    expect(getCharsetLength("abc")).toBe(CHARSET_SIZES.lowercase);
  });

  it("detects uppercase only", () => {
    expect(getCharsetLength("ABC")).toBe(CHARSET_SIZES.uppercase);
  });

  it("detects digits only", () => {
    expect(getCharsetLength("123")).toBe(CHARSET_SIZES.digits);
  });

  it("detects special characters only", () => {
    expect(getCharsetLength("!@#")).toBe(CHARSET_SIZES.special);
  });

  it("detects all character sets", () => {
    expect(getCharsetLength("aA1!")).toBe(
      CHARSET_SIZES.lowercase +
        CHARSET_SIZES.uppercase +
        CHARSET_SIZES.digits +
        CHARSET_SIZES.special,
    );
  });

  it("does not count non-ASCII characters as special", () => {
    expect(getCharsetLength("中文")).toBe(0);
  });

  it("does not count space as special", () => {
    expect(getCharsetLength(" ")).toBe(0);
  });

  it("counts underscore as special (it is in SPECIAL charset)", () => {
    expect(getCharsetLength("_")).toBe(CHARSET_SIZES.special);
  });

  it("counts actual SPECIAL characters correctly", () => {
    for (const ch of SPECIAL) {
      expect(getCharsetLength(ch)).toBe(CHARSET_SIZES.special);
    }
  });
});

describe("getCrackTimeEstimation", () => {
  it("returns zero entropy for empty password", () => {
    const result = getCrackTimeEstimation("");
    expect(result.entropy).toBe(0);
    expect(result.secondsToCrack).toBe(0);
    expect(result.score).toBe(0);
    expect(result.charsetLength).toBe(0);
  });

  it("returns zero entropy for password with unrecognized characters", () => {
    const result = getCrackTimeEstimation("中文");
    expect(result.entropy).toBe(0);
    expect(result.charsetLength).toBe(0);
  });

  it("calculates entropy correctly for lowercase password", () => {
    const result = getCrackTimeEstimation("abc");
    const expectedEntropy = Math.log2(26) * 3;
    expect(result.entropy).toBeCloseTo(expectedEntropy, 5);
  });

  it("calculates entropy correctly for mixed charset", () => {
    const result = getCrackTimeEstimation("aA1!");
    const charsetLen =
      CHARSET_SIZES.lowercase +
      CHARSET_SIZES.uppercase +
      CHARSET_SIZES.digits +
      CHARSET_SIZES.special;
    const expectedEntropy = Math.log2(charsetLen) * 4;
    expect(result.entropy).toBeCloseTo(expectedEntropy, 5);
  });

  it("caps score at 1", () => {
    const result = getCrackTimeEstimation("a".repeat(200));
    expect(result.score).toBe(1);
  });

  it("respects custom guessesPerSecond", () => {
    const r1 = getCrackTimeEstimation("abc", 1e9);
    const r2 = getCrackTimeEstimation("abc", 1e10);
    expect(r2.secondsToCrack).toBeCloseTo(r1.secondsToCrack / 10, 5);
  });
});

describe("formatDuration", () => {
  it("returns 瞬间 for very small values", () => {
    expect(formatDuration(0)).toBe("瞬间");
    expect(formatDuration(0.0005)).toBe("瞬间");
  });

  it("returns 不到 1 秒 for sub-second values", () => {
    expect(formatDuration(0.5)).toBe("不到 1 秒");
    expect(formatDuration(1)).toBe("不到 1 秒");
  });

  it("formats seconds", () => {
    expect(formatDuration(30)).toBe("30 秒");
  });

  it("formats minutes and seconds", () => {
    expect(formatDuration(125)).toBe("2 分钟 5 秒");
  });

  it("formats hours and minutes", () => {
    expect(formatDuration(3661)).toBe("1 小时 1 分钟");
  });

  it("formats days and hours", () => {
    expect(formatDuration(90000)).toBe("1 天 1 小时");
  });

  it("formats years", () => {
    expect(formatDuration(31536000)).toBe("1 年");
  });

  it("formats very large durations", () => {
    const result = formatDuration(1e20);
    expect(result).toContain("千年");
  });
});

describe("getStrengthLevel", () => {
  it("returns 极弱 for score < 0.25", () => {
    expect(getStrengthLevel(0).label).toBe("极弱");
    expect(getStrengthLevel(0.24).label).toBe("极弱");
  });

  it("returns 弱 for 0.25 <= score < 0.5", () => {
    expect(getStrengthLevel(0.25).label).toBe("弱");
    expect(getStrengthLevel(0.49).label).toBe("弱");
  });

  it("returns 中等 for 0.5 <= score < 0.75", () => {
    expect(getStrengthLevel(0.5).label).toBe("中等");
    expect(getStrengthLevel(0.74).label).toBe("中等");
  });

  it("returns 强 for score >= 0.75", () => {
    expect(getStrengthLevel(0.75).label).toBe("强");
    expect(getStrengthLevel(1).label).toBe("强");
  });

  it("percent equals score * 100", () => {
    expect(getStrengthLevel(0.3).percent).toBe(30);
    expect(getStrengthLevel(0.6).percent).toBe(60);
  });
});

describe("generatePassword", () => {
  it("generates password of specified length", () => {
    const pw = generatePassword(16, true, true, true, true);
    expect(pw.length).toBe(16);
  });

  it("generates lowercase only when only useLower is true", () => {
    const pw = generatePassword(100, true, false, false, false);
    expect(pw).toMatch(/^[a-z]+$/);
  });

  it("generates uppercase only when only useUpper is true", () => {
    const pw = generatePassword(100, false, true, false, false);
    expect(pw).toMatch(/^[A-Z]+$/);
  });

  it("generates digits only when only useDigits is true", () => {
    const pw = generatePassword(100, false, false, true, false);
    expect(pw).toMatch(/^[0-9]+$/);
  });

  it("generates special only when only useSpecial is true", () => {
    const pw = generatePassword(100, false, false, false, true);
    for (const ch of pw) {
      expect(SPECIAL).toContain(ch);
    }
  });

  it("falls back to lowercase when all flags are false", () => {
    const pw = generatePassword(100, false, false, false, false);
    expect(pw).toMatch(/^[a-z]+$/);
  });

  it("generates empty string for length 0", () => {
    const pw = generatePassword(0, true, true, true, true);
    expect(pw).toBe("");
  });

  it("produces different passwords on successive calls (probabilistic)", () => {
    const pw1 = generatePassword(32, true, true, true, true);
    const pw2 = generatePassword(32, true, true, true, true);
    expect(pw1).not.toBe(pw2);
  });
});
