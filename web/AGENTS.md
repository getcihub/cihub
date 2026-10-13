# web

The dashboard: a single-page application in TypeScript, built by Vite and embedded in the `cihub` binary by the
`web` role. It reads and acts only through the public API; it holds no queue logic and no rule that the server
does not also check.

## Stack (Q9, decided)

- TypeScript in strict mode (`strict`, `noUncheckedIndexedAccess`); no `any`, no non-null assertion `!`, no
  `@ts-ignore`. Not `exactOptionalPropertyTypes`: the generated client does not compile with it.
- React, Vite, and the TanStack libraries: Router for the routes, Query for the data of the server, Form for the
  forms. Table only for a table that the client sorts or filters; the server already orders the items.
- The client of the API is generated from `web/openapi.json`, the document that `/api/v1/openapi.json` serves,
  by `@hey-api/openapi-ts`, with its plugin `@tanstack/react-query` (`queryOptions`, `mutationOptions`, query
  keys). The generated files are never edited by hand; a change of the API regenerates them in the same change
  (`mise run web:generate`), and `mise run web:generate-check` fails otherwise. The routes outside the API
  (`/auth/*`, `specs/API.md`) are called with `fetch` in `src/domain/session/session.ts` only.
- Zod 4 for the search parameters and the forms, without adapter (Standard Schema).
- Vitest and Testing Library for the components; Playwright for the browser tests, against a server on the
  simulator (`cihub-scm`, feature `sim`), never the real SCM.
- Tailwind CSS 4 for every style, with `class-variance-authority` for the variants of a component and `clsx` with
  `tailwind-merge` for `cn()` (decided October 12, 2026, by the project lead). No CSS module, no CSS file other
  than `src/styles/globals.css`.
- `motion` for the animations (decided October 12, 2026, by the project lead), through `m.*` components under
  `LazyMotion strict` (`src/components/ui/motion.tsx`): a `motion.*` component throws.
- oxfmt formats every file of the dashboard (`.oxfmtrc.json`) and sorts the Tailwind classes, in `className`,
  `cn()` and `cva()` (decided October 12, 2026, by the project lead). Its version is exact: a new one can change
  the format, so its update shows the files that it changes. It skips the generated files (`src/client/`,
  `src/routeTree.gen.ts`, `openapi.json`), which the generator writes.
- A new dependency of `package.json` is proposed to the project lead, as a crate is.

## Layout

`src/` has six layers; an import goes down, never up, and a page never imports another page. ESLint
(`no-restricted-imports`) refuses the others, and a relative import that leaves its folder: another folder is
imported through `@/` (`@/domain/queue/queries`).

| Folder           | What it holds                                                                                                  | It imports                                                            |
| ---------------- | -------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------- |
| `pages/`         | one folder per page (see "Pages"): its route, its states, and the components and hooks that only it uses       | its own folder, `components/`, `hooks/`, `domain/`, `lib/`, `client/` |
| `components/`    | the components that two pages or more share, by subject (`frame/`, `queue/`, `pr/`, `mine/`, `errors/`)        | another component, `hooks/`, `domain/`, `lib/`, `client/`             |
| `hooks/`         | the hooks (`use-*.ts`) that two pages or more share: the reads of the API, the session, the time               | `domain/`, `lib/`, `client/`                                          |
| `domain/`        | one folder per resource of the API (`queue/`, `pr/`, `session/`, `mine/`): its reads, actions, words; no React | `lib/`, `client/`, another `domain/` folder                           |
| `components/ui/` | the components of the design system, which know nothing of CIHub (see "Interface")                             | `lib/`                                                                |
| `lib/`           | the functions that know nothing of CIHub: `cn`, the query client, the times, the plurals                       | nothing of `src/`                                                     |

`client/` is generated, `styles/` holds the design system, `test/` the helpers of the tests. A component or a page
and its test sit side by side. A component or a hook that only one page uses sits in the folder of that page; it
moves to `components/` or `hooks/` when a second page needs it. Every file name is in kebab-case
(`queue-graph.tsx`, `use-org-queues.ts`). There is no barrel file (`index.ts`): an import names the file.
`components/icons/` holds the icons, one file per icon (`pause-icon.tsx`), each an `<svg>` in `currentColor`
that its slot sizes.

## Pages

A page is a folder of `src/pages/`, named after the page (`queue/`, `org-queues/`), with these files:

| File            | What it holds                                                                                                 |
| --------------- | ------------------------------------------------------------------------------------------------------------- |
| `route.tsx`     | the route: `validateSearch`, the `loader` (its reads, `redirect()`, `notFound()`), and the file of each state |
| `page.tsx`      | `component`, the page, which puts the components together; `layout.tsx` instead for a layout (`org/`)         |
| `loading.tsx`   | `pendingComponent`, the skeleton of the page                                                                  |
| `error.tsx`     | `errorComponent`, what the page keeps when a read fails                                                       |
| `not-found.tsx` | `notFoundComponent`, when the page has its own                                                                |
| `components/`   | the components that only this page uses                                                                       |
| `use-*.ts`      | the hooks that only this page uses                                                                            |

A state that the page shares with others comes from `components/` (`NotFoundPage`, `ServerErrorPage`,
`QueueSkeleton`), and the page has no file for it. `src/pages/redirects/` holds the routes that only redirect.

## Routes

- `src/routes.ts` is the tree of the addresses (`@tanstack/virtual-file-routes`, decided October 12, 2026, by the
  project lead): each address and the `route.tsx` of its page, in `src/pages/` (`routesDirectory`).
  `@tanstack/router-plugin/vite`, placed before `@vitejs/plugin-react`, generates `routeTree.gen.ts` from it, with
  `autoCodeSplitting`. A new page adds its line to `routes.ts` and its folder. The layout of a page is the route
  above it in the tree (`org/layout.tsx` for the pages of an organization).
- A `route.tsx` holds the configuration of its route, never a component. A page reads the params and the search of
  its route with `getRouteApi(id)`, never by importing the route.
- An address without a page of its own redirects to the page above it (`src/pages/redirects/`).
- The URL holds the state that a link must keep: the organization, the queue, the open PR, the tab. Every search
  parameter goes through `validateSearch` with a Zod schema.
- A link is a typed `<Link to=…>` or `navigate({ to })`; never a path built from strings.
- The router context carries the `QueryClient`. The layout of the signed-in pages (`src/pages/org/route.tsx`) reads
  the person of the session (`/user`, `/user/orgs` and `/instance`, the `Me` of `src/domain/session/session.ts`) in
  its loader, and redirects to the sign-in page on `401`.

## Data of the server

- Every read goes through TanStack Query with the generated `queryOptions`; never a `fetch` by hand, never a copy
  of the data of the server in `useState`, a context or a store.
- A route loader starts its reads with `queryClient.query(...)` (`ensureQueryData` is deprecated) and awaits the
  ones that the whole page shows; the component reads them with `useSuspenseQuery`. No request waterfall: a loader
  starts the independent reads together. A read of a part of the page that changes without changing the page (the
  items of a tab) starts in the loader without being awaited, and the part waits for it in a `<Suspense>` of its
  own.
- A link starts the reads of its page when the pointer rests on it (`defaultPreload: "intent"`); TanStack Query
  says what is still fresh (`defaultPreloadStaleTime: 0`).
- The stream `/orgs/{org}/stream` (`src/domain/queue/live.ts`, followed by `src/pages/org/use-queue-news.ts`) invalidates the query keys of the queue that
  changed, gathered over half a second, and every read of the organization each time it opens; it opens again
  after an error; no polling.
- A mutation invalidates the keys that it changes in `onSettled`. No optimistic update of the queue: the API
  answers `202` and the queue decides at its next reconciliation; the page shows the request as sent until the
  stream signals the change.
- An error is the `Problem` of the API: the page decides on its `code`, never on `detail` or on the status alone.

## Loading, errors and actions

- A page shows its skeleton (`components/ui/skeleton.tsx`, the shapes of the mockups v5) when its reads take more than 200
  ms, then for 500 ms at least, so that it never flashes (`defaultPendingMs`, `defaultPendingMinMs`). The frame
  stays: only the page waits. While the frame of an organization loads, it shows the skeleton of the page that the
  address opens. A part that waits on its own (a tab, the run of a PR) shows the skeleton of that part.
- A failed read keeps what the page can still show (the frame, the head of a queue, the drawer) and says, in a card,
  what the server answered, with "Try again" (`components/errors/server-error.tsx`). A `401` sends the person to the
  sign-in and back to the page, once a new read of the person says that the session ended; else it shows as a
  failure, so that the sign-in never loops.
- An action that runs says so in its button: a spinner and what runs ("Submitting"), with `isPending` and
  `pendingText` of `Button`.
- The skeleton of a page sits in its `loading.tsx`, which never imports `page.tsx`, `layout.tsx` or `error.tsx`:
  `autoCodeSplitting` keeps a `pendingComponent` in the entry chunk, and a skeleton imported from the file of its
  page would bring the whole page into it.
- `mise run web:shots` captures these states too: a view whose read never answers shows the skeleton.

## Animations

- The animations follow the mockups v5: the drawer of a PR slides in from the right and its scrim fades in; a page
  fades in as it opens; a menu fades in and out; a modal fades in; a notice slides in and out. Their timings are
  the `transitions` of `src/components/ui/motion.tsx` (the `ease` of CSS, 120 to 200 ms); a component never writes its own.
- An element that leaves animates out with `<AnimatePresence>` when its component owns its state (a menu, a
  notice). The drawer is a route: Close, Escape and a click beside it slide it out first, then go back to the page.
- A person who asks for less motion gets no animation at all, fades included (`skipAnimations` of
  `MotionProvider`); the tests and `mise run web:shots` ask for it too, so that a capture never depends on a frame.
- The features of `motion` load with the first page: a page and the drawer start hidden until they animate, and a
  later chunk would leave them hidden. They cost about 27 kB gzip at start.

## Vocabulary and rights

- The page shows `state_description` and the descriptions that the API gives; it decides on the codes
  (`state_reason`, `kind`, `code`). It never writes its own sentence for a reason of the closed vocabulary.
- The role of the person (`/user/orgs`) only hides or disables a button; the server refuses what the role does not
  allow.
- Every text is in English, with the words of `specs/DOMAIN.md`. The texts of the dashboard may name GitHub, as
  the mockups do ("Continue with GitHub"), an exception to the neutral vocabulary of the code that the project
  lead accepted: the name will come from the API when a second SCM arrives.

## Interface

- `src/styles/globals.css` is the design system: the tokens of the mockups v5 in two themes (`data-theme`), and
  the Tailwind theme that makes them utilities. The theme holds these tokens only: a color, a radius, a shadow, a
  font or a size of the Tailwind palette does not exist, so `bg-white` or `text-gray-500` is a build without the
  class.
- Colors by role, never by hue: `bg-card`, `text-muted-foreground`, `border-destructive-border`, and the `info`,
  `warning`, `destructive`, `success` tones with their `-muted` (background) and `-border`. The dark theme comes
  from the tokens: a component names a theme (`dark:`, which follows `data-theme`) only where no token can tell
  the two apart.
- No arbitrary value (`text-[13px]`, `bg-[#fff]`, `rounded-[5px]`) for a color, a size of text, a radius or a
  shadow: use a token of `globals.css`, or add one. An arbitrary value stays for a layout that only one component
  has (`grid-cols-[auto_1fr]`).
- A class goes on the element that it styles, never on a parent through a descendant selector (`[&_p]:`), but for
  the icons that a component sizes in its own slot (`[&>svg]:size-4`): a part that repeats is a component
  (`DrawerSection`, `Setting`). A repeated look is a component of `src/components/ui/`, not a list
  of classes copied from page to page.
- Every action works with the keyboard; `Escape` closes a menu, a drawer or a modal. A page has its loading, empty
  and error states.

### A component of `src/components/ui/`

`src/components/ui/` holds the components that every page shares (`alert`, `badge`, `button`, `card`, `menu`, `modal`,
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
   `role="status"`), and the keyboard of `src/components/ui/dialog.ts` for what opens over the page.
7. It imports `@/lib/` only: no route, no read of the API, no type of `@/client/`, no word of `specs/DOMAIN.md`. A
   component that knows the domain (the badge of a state) goes in another folder of `@/components/` and gives the generic one its
   variant.

When a component does not fit: pass `className`; compose it in the page that needs it; add a variant here only
when a second page needs it. A change of a component is a change of every page that shows it: compare the views
before and after with `mise run web:shots-diff`, and explain each view that differs.

## Commands

`mise run lint` and `mise run check` cover the dashboard: type check, ESLint (`typescript-eslint`,
`react-hooks`, `@tanstack/eslint-plugin-query`, `@tanstack/eslint-plugin-router`), `oxfmt --check`, Vitest.
`mise run web:fmt` formats the dashboard; the pre-commit hook (`.lefthook.toml`) formats the files that a
commit stages, as `cargo fmt` does for the Rust code.
`mise run web:dev` serves it with Vite in front of a `cihub server start` (`CIHUB_URL`, `http://localhost:8080`);
`mise run web:build` builds `web/dist`, which the next build of `cihub-web` embeds; `mise run web:generate`
writes `openapi.json` from the code (`cargo run -p cihub-cli --example openapi`) and the client in `src/client`;
`mise run web:e2e` runs the browser tests of `e2e/` (Playwright) against `cargo run -p cihub-cli --example e2e`, a
server on the simulator with a fresh database, which runs the queues, submits PRs of `acme/web` at its start,
passes each test commit of `acme/api`, comments `/cihub merge` on a PR at `/sim/merge`, and gives the last
comment of a PR at `/sim/comment`.

`mise run web:shots` captures the views of `e2e/visual/views.ts` (each page, in the states that the simulator
cannot show, in both themes and in a narrow window) from a build of the dashboard, against the API of
`e2e/visual/api.ts` and a fixed clock, into `e2e/__screenshots__/head`, which git ignores: two captures of the
same
build are the same. `mise run web:shots-diff [REF]` (`origin/main` by default) builds REF in a worktree, captures
both, and writes into `e2e/__screenshots__/diff` the changed pixels of each view that differs (`changes/`),
`report.md`, and `index.html`, which shows each view before and after: side by side, split, blend, or its changed
pixels, zoomed on the change. Its viewer (`e2e/visual/viewer/`) uses the design system and the components of
`src/`, built as one script, as a page opened from the disk runs no module. The fixtures use the types of `src/client`: a change of the API that they no longer
follow fails the type check. A new page or a new state adds its view to `views.ts`.

Specs: `specs/API.md`, `specs/SECURITY.md`, `specs/LOTS.md` (lots 10a to 10d).
