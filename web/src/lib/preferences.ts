// What the browser keeps for the person: the theme (index.html reads it
// before the first paint) and the organization opened last, which `/` opens.
const THEME_KEY = "theme";
const LAST_ORG_KEY = "org";

export function toggleTheme(): void {
  const root = document.documentElement;
  const next = root.dataset.theme === "dark" ? "light" : "dark";

  root.dataset.theme = next;
  localStorage.setItem(THEME_KEY, next);
}

export function lastOrgSlug(): string | null {
  return localStorage.getItem(LAST_ORG_KEY);
}

export function keepLastOrg(slug: string): void {
  localStorage.setItem(LAST_ORG_KEY, slug);
}
