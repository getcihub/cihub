// What the browser keeps for the person: the theme (index.html reads it
// before the first paint) and the organization opened last, which `/` opens.
const THEME_KEY = "theme";
const LAST_ORG_KEY = "org";

export function toggleTheme(): void {
  chooseTheme(document.documentElement.dataset.theme === "dark" ? "light" : "dark");
}

/** "system" keeps no key, so that index.html follows the setting of the system. */
export type Theme = "light" | "dark" | "system";

export function chosenTheme(): Theme {
  const stored = localStorage.getItem(THEME_KEY);
  return stored === "light" || stored === "dark" ? stored : "system";
}

export function chooseTheme(theme: Theme): void {
  if (theme === "system") {
    localStorage.removeItem(THEME_KEY);
  } else {
    localStorage.setItem(THEME_KEY, theme);
  }

  const isDark = theme === "system" ? matchMedia("(prefers-color-scheme: dark)").matches : theme === "dark";
  document.documentElement.dataset.theme = isDark ? "dark" : "light";
}

export function lastOrgSlug(): string | null {
  return localStorage.getItem(LAST_ORG_KEY);
}

export function keepLastOrg(slug: string): void {
  localStorage.setItem(LAST_ORG_KEY, slug);
}
