export const themes = [
  {
    id: "dark",
    name: "Dark",
    mode: "dark",
    colors: [
      "#111413",
      "#151817",
      "#1b1f1d",
      "#242b27",
      "#343d37",
      "#e6e9e7",
      "#9ca69f",
      "#89978e",
      "#bed9c5",
      "#182c1f",
    ],
  },
  {
    id: "midnight",
    name: "Midnight",
    mode: "dark",
    colors: [
      "#0e1420",
      "#121b2b",
      "#182337",
      "#22314a",
      "#35465f",
      "#e4ecf9",
      "#a1b3ce",
      "#8ea2bf",
      "#9cc5ff",
      "#13253f",
    ],
  },
  {
    id: "ocean",
    name: "Ocean",
    mode: "dark",
    colors: [
      "#0d191c",
      "#112025",
      "#172b30",
      "#203b42",
      "#335259",
      "#e0f0ef",
      "#a0bdbc",
      "#87aaa9",
      "#87d9d1",
      "#103530",
    ],
  },
  {
    id: "violet",
    name: "Violet",
    mode: "dark",
    colors: [
      "#17131e",
      "#1e1828",
      "#292034",
      "#382c46",
      "#4b3e59",
      "#eee7f7",
      "#baaacb",
      "#a997ba",
      "#c9aff1",
      "#302044",
    ],
  },
  {
    id: "ember",
    name: "Ember",
    mode: "dark",
    colors: [
      "#1b1512",
      "#221a16",
      "#2d231d",
      "#3c2e25",
      "#534034",
      "#f1e9e1",
      "#c5b1a0",
      "#b49d89",
      "#efb781",
      "#3d2715",
    ],
  },
  {
    id: "rose",
    name: "Rose",
    mode: "dark",
    colors: [
      "#1c1319",
      "#251a22",
      "#30222c",
      "#402f3a",
      "#56404e",
      "#f4e8ef",
      "#c7adbd",
      "#b59aab",
      "#edafc9",
      "#3d2030",
    ],
  },
  {
    id: "light",
    name: "Light",
    mode: "light",
    colors: [
      "#f4f5f1",
      "#eceee8",
      "#e6eae2",
      "#dce3d8",
      "#c6cfc0",
      "#253128",
      "#526355",
      "#566851",
      "#356743",
      "#ffffff",
    ],
  },
  {
    id: "sand",
    name: "Sand",
    mode: "light",
    colors: [
      "#faf4e9",
      "#f0e7d8",
      "#eadeca",
      "#e1d1b9",
      "#cbb79a",
      "#3d3024",
      "#6c5945",
      "#70583f",
      "#925634",
      "#ffffff",
    ],
  },
] as const;
export type ThemeId = (typeof themes)[number]["id"];
const tokens = [
  "bg",
  "sidebar",
  "surface",
  "surface-hover",
  "line",
  "text",
  "muted",
  "faint",
  "accent",
  "on-accent",
] as const;
export function resolveTheme(value: unknown) {
  return themes.find((theme) => theme.id === value) ?? themes[0];
}
export function themeProperties(value: unknown): Record<string, string> {
  const theme = resolveTheme(value);
  return {
    ...Object.fromEntries(
      tokens.map((key, index) => [`--${key}`, theme.colors[index]]),
    ),
    "--shadow": theme.mode === "light" ? "#25312820" : "#00000055",
    "--danger": theme.mode === "light" ? "#a03c30" : "#f0ab9d",
    "--color-scheme": theme.mode,
  };
}
export function applyTheme(root: HTMLElement, value: unknown) {
  const theme = resolveTheme(value);
  root.dataset.theme = theme.id;
  root.dataset.appearance = theme.mode;
  for (const [key, color] of Object.entries(themeProperties(theme.id)))
    root.style.setProperty(key, color);
  root.ownerDocument
    .querySelector('meta[name="theme-color"]')
    ?.setAttribute("content", theme.colors[0]);
}
