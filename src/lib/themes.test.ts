import { describe, expect, it } from "vitest";
import { resolveTheme, themes, themeProperties } from "./themes";

function luminance(color: string) {
  const rgb = color
    .slice(1)
    .match(/../g)!
    .map((hex) => parseInt(hex, 16) / 255)
    .map((c) => (c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4));
  return rgb[0] * 0.2126 + rgb[1] * 0.7152 + rgb[2] * 0.0722;
}
function contrast(a: string, b: string) {
  const [light, dark] = [luminance(a), luminance(b)].sort((a, b) => b - a);
  return (light + 0.05) / (dark + 0.05);
}
describe("color theme compatibility and legibility", () => {
  it("retains legacy preferences and gracefully resolves missing, removed or invalid themes", () => {
    expect(resolveTheme("dark").id).toBe("dark");
    expect(resolveTheme("light").mode).toBe("light");
    for (const theme of themes) expect(resolveTheme(theme.id)).toBe(theme);
    for (const value of [
      undefined,
      null,
      {},
      "future-unknown-theme",
      "__proto__",
      "",
    ])
      expect(resolveTheme(value).id).toBe("dark");
    expect(new Set(themes.map((theme) => theme.id)).size).toBe(themes.length);
  });
  it("keeps small text and primary actions readable on each palette's surfaces", () => {
    for (const theme of themes) {
      const tokens = themeProperties(theme.id);
      for (const surface of ["--bg", "--sidebar", "--surface"]) {
        for (const text of ["--text", "--muted", "--faint", "--danger"])
          expect(
            contrast(tokens[text], tokens[surface]),
            `${theme.name}: ${text} on ${surface}`,
          ).toBeGreaterThanOrEqual(4.5);
      }
      expect(
        contrast(tokens["--on-accent"], tokens["--accent"]),
        `${theme.name}: primary button`,
      ).toBeGreaterThanOrEqual(4.5);
    }
  });
});
