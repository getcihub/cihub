# @cihub/ui

The design system of the console (`apps/console/`) and of the website (`apps/website/`): the tokens, the Tailwind
theme, the components and their hooks, `cn()`, the logo and the icons. It holds no page, no data and no knowledge
of CIHub's domain.

- TypeScript and CSS source, without a build: each app compiles it (Vite; `transpilePackages` for Next.js) and
  scans it for its classes (`@source`). `src/index.ts` is the entry point of the package, the only barrel file;
  `@cihub/ui/utc-days` gives the UTC days of `RangeCalendar` without React, for the domain of the console.
- `src/tokens.css`: the colors of the mockups v5, by role (`--card`, `--muted-foreground`, `--destructive-border`),
  in a light theme and a dark one. The console marks the dark theme with `data-theme="dark"`, Fumadocs with the
  class `dark`: both select it. A new color is a token here, in both themes.
- `src/theme.css`: the Tailwind theme of the components, and the utilities that they use (`px-page`,
  `bg-shimmer`). It replaces the scales of Tailwind with the tokens and the sizes, radii and fonts of the mockups,
  so the website, whose Fumadocs needs these scales, does not import it: a component shows as designed only in
  the console.
- `src/icons/`: one file per icon (`pause-icon.tsx`), each an `<svg>` in `currentColor` that its slot sizes.
  `src/mark.tsx`: the logo, in the colors of the tokens.

## A component

`src/` holds the components that every page of the console shares (`alert`, `badge`, `button`, `card`, `menu`, `modal`,
`page`…), which know nothing of CIHub. `alert.tsx` is the reference.

1. One file per component, with its parts as functions of their own: `Alert`, `AlertContent`, `AlertTitle`,
   `AlertDescription`. The caller composes the parts; no `title=` prop does it for them.
2. Its props are the props of the element that it renders, plus its variants:
   `ComponentProps<"div"> & VariantProps<typeof alertVariants>`. The other props go to the element with
   `{...props}`, so `aria-*`, `id`, `role` and the handlers work with no extra code.
3. It takes `className` and merges it last with `cn()`, so that a class of the caller wins over its own.
4. Its variants are a `cva()`: the classes of the base, one entry per variant, then `defaultVariants`. Their type
   comes from the declaration (`VariantProps`). A `<Link>` that looks like a component takes the classes of its
   variants function (`buttonVariants()`, `cardVariants()`).
5. Each part carries `data-slot` (`alert`, `alert-title`), and the root `data-variant`: a test finds it without a
   class.
6. It has the role of what it is (`Alert` is `role="alert"`; a caller that informs without interrupting passes
   `role="status"`), and the keyboard of `src/hooks/use-dialog.ts` for what opens over the page.
7. It imports the files of this package only: no route, no read of the API, no type of the client of the console, no
   word of `specs/DOMAIN.md`. A component that knows the domain (the badge of a state) goes in `components/` of
   the console and gives the generic one its variant.
8. A file that uses a hook of React other than `useId`, a handler or `motion` starts with `"use client"`: the
   website imports `@cihub/ui` from its server components, and Next.js refuses a client API there.
9. It is exported from `src/index.ts`, and its test (`button.test.tsx`) sits beside it.

When a component does not fit: pass `className`; compose it in the page that needs it; add a variant here only
when a second page needs it. A change of a component is a change of every page that shows it: compare the views
before and after with `mise run console:shots-diff`, and explain each view that differs.

## Commands

`mise run ui:lint` type checks, lints and checks the format; `mise run ui:test` runs the tests of the components
(Vitest, Testing Library); `mise run ui:fmt` formats.
