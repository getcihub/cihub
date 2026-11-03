# @cihub/theme

The look of CIHub as CSS, for the console and the website: no TypeScript, no component.

- `tokens.css`: the colors of the mockups v32, by role (`--card`, `--muted-foreground`, `--destructive-border`), in
  a light theme and a dark one. The console marks the dark theme with `data-theme="dark"`, Fumadocs with the class
  `dark`: both select it. A new color is a token here, in both themes.
- `tailwind.css`: the Tailwind theme of the console, and the utilities that its components use (`px-page`,
  `bg-shimmer`). It replaces the scales of Tailwind with the tokens and the sizes, radii and fonts of the mockups,
  so the website, whose Fumadocs needs these scales, imports `tokens.css` only.

`mise run packages:lint` checks the format; `mise run packages:fmt` formats.
