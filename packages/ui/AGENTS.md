# @cihub/ui

The components of the console (`apps/console/`): buttons, cards, menus, modals, the calendar, and the hooks that they
need. They know nothing of CIHub. The package holds no token, no icon and no date: those are the other packages of
`packages/`.

| Package        | What it holds                                                                               | It imports                      |
| -------------- | ------------------------------------------------------------------------------------------- | ------------------------------- |
| `@cihub/theme` | `tokens.css`, the colors, radii and fonts in two themes; `tailwind.css`, the Tailwind theme | nothing                         |
| `@cihub/icons` | one React component per icon, and the logo (`brand/mark`)                                   | React                           |
| `@cihub/utils` | the functions without React that more than one package needs (`utc-days`)                   | nothing                         |
| `@cihub/ui`    | the components, their hooks and `cn()`                                                      | `@cihub/utils`, React, `motion` |

- TypeScript source, without a build: each app compiles it (Vite) and scans it for its classes (`@source`).
- No barrel file: an import names the file, `@cihub/ui/components/button`, `@cihub/ui/hooks/use-flip`,
  `@cihub/ui/lib/cn`. The `exports` of `package.json` map these paths.
- `src/components/`: one file per component and its test (`button.test.tsx`). `src/hooks/`: the hooks of the
  components (`use-dialog`, `use-close-outside`, `use-flip`). `src/lib/cn.ts`: `cn()`, clsx with tailwind-merge.
- `react` and `motion` are peer dependencies: `LazyMotion strict` of `components/motion.tsx` needs the one
  instance of `motion` that the app installs.

## A component

`src/components/` holds the components that every page of the console shares (`alert`, `badge`, `button`, `card`,
`menu`, `modal`, `page`…), which know nothing of CIHub. `alert.tsx` is the reference.

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
7. It imports the files of this package, `@cihub/utils` and its own dependencies only: no route, no read of the
   API, no type of the client of the console, no word of `specs/DOMAIN.md`; an icon comes from the caller. A component that knows the domain (the badge of a state) goes in `components/` of
   the console and gives the generic one its variant.
8. A file that uses a hook of React other than `useId`, a handler or `motion` starts with `"use client"`, so that a
   server component of Next.js can import it.
9. Its test (`button.test.tsx`) sits beside it.

When a component does not fit: pass `className`; compose it in the page that needs it; add a variant here only
when a second page needs it. A change of a component is a change of every page that shows it: compare the views
before and after with `mise run console:shots-diff`, and explain each view that differs.

## Commands

`mise run packages:lint` type checks, lints and checks the format of every package of `packages/`;
`mise run packages:test` runs their tests (Vitest, Testing Library); `mise run packages:fmt` formats them.
