# @cihub/icons

The icons and the logo of CIHub, as React components, for the console and the website.

- One file per icon, named after what it shows (`src/pause.tsx`, `PauseIcon`), imported by its path:
  `@cihub/icons/pause`. The logo is `src/brand/mark.tsx`, `@cihub/icons/brand/mark`.
- An icon is an `<svg>` in `currentColor`, `aria-hidden` by default, whose props are the props of `<svg>`
  (`ComponentProps<"svg">`, passed with `{...props}` last): its slot sizes it with `className`, and a caller can
  name it with `aria-label`.
- No hook but `useId`, no handler: a server component of Next.js imports an icon as it is.
- An icon follows the mockups v5; a new one is drawn on the grid of the others (16 by 16, stroke 1.5 to 1.8).

`mise run packages:lint` type checks, lints and checks the format.
