# web

The dashboard: a single-page application in TypeScript, built by Vite and embedded in the `cihub` binary by the
`web` role. It reads and acts only through the public API; it holds no queue logic and no rule that the server
does not also check.

## Stack (Q9, decided)

- TypeScript in strict mode (`strict`, `noUncheckedIndexedAccess`); no `any`, no `unknown`, no non-null assertion
  `!`, no `@ts-ignore`. ESLint refuses `any` and `unknown`; `unknown` stays only where no type can be known (a
  thrown value, a JSON that the API gives no schema), with a disable comment that says why. Not
  `exactOptionalPropertyTypes`: the generated client does not compile with it.
- React, Vite, and the TanStack libraries: Router for the routes, Query for the data of the server, Form for the
  forms. Table only for a table that the client sorts or filters; the server already orders the items.
- The client of the API is generated from `schema/openapi/v1.json`, the document that `/api/v1/openapi.json` serves,
  by `@hey-api/openapi-ts`, with its plugin `@tanstack/react-query` (`queryOptions`, `mutationOptions`, query
  keys). The generated files are never edited by hand; a change of the API regenerates them in the same change
  (`mise run console:generate`), and `mise run console:generate-check` fails otherwise. The routes outside the API
  (`/auth/*`, `specs/API.md`) are called with `fetch` in `src/domain/session/session.ts` only.
- Zod 4 for the search parameters and the forms, without adapter (Standard Schema).
- Vitest and Testing Library for the components; Playwright for the browser tests, against a server on the
  simulator (`cihub-scm`, feature `sim`), never the real SCM.
- Tailwind CSS 4 for every style, with `class-variance-authority` for the variants of a component and `clsx` with
  `tailwind-merge` for `cn()` (decided October 12, 2026, by the project lead). No CSS module, no CSS file other
  than `src/styles/globals.css` and the ones of `@cihub/theme`.
- `motion` for the animations (decided October 12, 2026, by the project lead), through `m.*` components under
  `LazyMotion strict` (`components/motion.tsx` of `@cihub/ui`): a `motion.*` component throws.
- oxfmt formats every file of the dashboard (`.oxfmtrc.json`) and sorts the Tailwind classes, in `className`,
  `cn()` and `cva()` (decided October 12, 2026, by the project lead). Its version is exact: a new one can change
  the format, so its update shows the files that it changes. It skips the generated files (`src/client/`,
  `src/routeTree.gen.ts`), which the generator writes.
- A new dependency of `package.json` is proposed to the project lead, as a crate is.

## Layout

`src/` has six layers; an import goes down, never up, and a page never imports another page. ESLint
(`no-restricted-imports`) refuses the others, and a relative import that leaves its folder: another folder is
imported through `@/` (`@/domain/queue/queries`).

| Folder        | What it holds                                                                                                                                          | It imports                                                            |
| ------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------- |
| `pages/`      | one folder per page (see "Pages"): its route, its states, and the components and hooks that only it uses                                               | its own folder, `components/`, `hooks/`, `domain/`, `lib/`, `client/` |
| `components/` | the components that two pages or more share, by subject (`queue/`, `pr/`, `mine/`, `errors/`), and `frame/`, the frame of the pages of an organization | another component, `hooks/`, `domain/`, `lib/`, `client/`             |
| `hooks/`      | the hooks (`use-*.ts`) that two pages or more share: the reads of the API, the session, the time                                                       | `domain/`, `lib/`, `client/`                                          |
| `domain/`     | one folder per resource of the API (`queue/`, `pr/`, `session/`, `mine/`), see below; no React                                                         | `lib/`, `client/`, another `domain/` folder                           |
| `lib/`        | the functions that know nothing of CIHub: the query client, the times, the plurals                                                                     | nothing of `src/`                                                     |

A folder of `domain/` splits its resource by role, each file when the resource has it:

| File         | What it holds                                                                                           |
| ------------ | ------------------------------------------------------------------------------------------------------- |
| `queries.ts` | one function per read, `<thing>Options(...)`, over the generated `queryOptions`; never a tuple of reads |
| `actions.ts` | `invalidate<Thing>(queryClient, ...)`: the reads that an action changes, by the generated `*QueryKey`   |
| `search.ts`  | the Zod schema of the search of a page, its defaults, and the values of its tabs (`TABS`, `PANES`)      |
| `words.ts`   | the words and the constants that the pages show (`COMMAND`)                                             |

Two files sit at the root of `domain/`: `problem.ts`, the failures of the API and the words of a refusal
(`failureOf`, `isProblem`, `refusalOf`), and `read-key.ts`, the fields of a query key for a predicate that a
`*QueryKey` cannot say ("every read of an organization", `live.ts`), whose `Operation` is typed by the generated SDK.

`client/` is generated, `styles/` holds the design system, `test/` the helpers of the tests. A component or a page
and its test sit side by side. A component or a hook that only one page uses sits in the folder of that page; it
moves to `components/` or `hooks/` when a second page needs it. Every file name is in kebab-case
(`queue-graph.tsx`, `use-org-queues.ts`). There is no barrel file (`index.ts`): an import names the file.
The shared packages of `packages/` (`packages/ui/AGENTS.md`) give the rest, each imported by the path of its file:
the components, their hooks and `cn()` from `@cihub/ui` (`@cihub/ui/components/button`, `@cihub/ui/lib/cn`), the
icons and the logo from `@cihub/icons` (`@cihub/icons/pause`, `@cihub/icons/brand/mark`), the UTC days from
`@cihub/utils/utc-days`, and the tokens and the Tailwind theme from `@cihub/theme`.

## Pages

A page is a folder of `src/pages/`, named after the page (`queue/`, `org-queues/`), with these files:

| File            | What it holds                                                                                                 |
| --------------- | ------------------------------------------------------------------------------------------------------------- |
| `route.tsx`     | the route: `validateSearch`, the `loader` (its reads, `redirect()`, `notFound()`), and the file of each state |
| `page.tsx`      | `component`, the page, which puts the components together; `layout.tsx` instead for a layout (`org/`)         |
| `loading.tsx`   | `pendingComponent`, the skeleton of the page                                                                  |
| `error.tsx`     | `errorComponent`, what the page keeps when a read fails                                                       |
| `not-found.tsx` | `notFoundComponent`, when the page has its own                                                                |
| `components/`   | the components that only this page uses; one folder per zone of the page once it holds about ten files        |
| `use-*.ts`      | the hooks that only this page uses                                                                            |

A state that the page shares with others comes from `components/` (`NotFoundPage`, `ServerErrorPage`,
`QueueSkeleton`), and the page has no file for it. `src/pages/redirects/` holds the routes that only redirect.

A zone is a part of the page that a person sees as one (`queue/components/head/`, `graph-pane/`, `health-pane/`):
a component sits in the zone that shows it, and a zone imports another one through `@/pages/<page>/components/`.
A component file holds one component and the small parts that only it uses; a part that another file needs, or a
file over about 200 lines, becomes a file of its own.

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
  the person of the session (`/user`, `/user/orgs` and `/instance`, the `Me` of `src/domain/session/queries.ts`) in
  its loader, and redirects to the sign-in page on `401`.

## Data of the server

- Every read goes through TanStack Query with the generated `queryOptions`, through the function of
  `domain/<resource>/queries.ts`; a page, a component or a hook never calls a generated `*Options` itself, so that a
  read changes in one place. Never a `fetch` by hand, never a copy of the data of the server in `useState`, a
  context or a store.
- A route loader starts its reads with `queryClient.query(...)` (`ensureQueryData` is deprecated), in three steps
  (`src/pages/queue/route.tsx`): it decides `notFound()` or `redirect()` on the `code` of a `Problem`; it starts,
  without awaiting them, the reads of a part that waits in a `<Suspense>` of its own (the items of a tab); then it
  awaits the reads that the whole page shows together, with `Promise.allSettled`. The component reads them with
  `useSuspenseQuery`, which throws a failed read to `error.tsx`. No request waterfall: a loader starts the
  independent reads together.
- A link starts the reads of its page when the pointer rests on it (`defaultPreload: "intent"`); TanStack Query
  says what is still fresh (`defaultPreloadStaleTime: 0`).
- The stream `/orgs/{org}/stream` (`src/domain/queue/live.ts`, followed by `src/pages/org/use-queue-news.ts`) invalidates the query keys of the queue that
  changed, gathered over half a second, and every read of the organization each time it opens; it opens again
  after an error; no polling, except the Health of a queue: its facts come a few seconds after the change, so it
  reads them again every 30 seconds while it is open, and not at each change; and a page that waits for a read of
  the SCM (a setup that the server reads, the installations after "Check again"): it reads again every 2 seconds,
  for 60 seconds at most (`readAgainWhile` of `src/domain/setup/read-again.ts`).
- A mutation is `useMutation({ ...<operation>Mutation(), onSettled: () => invalidate<Thing>(queryClient, ...) })`:
  the generated options, and the reads that it changes from `domain/<resource>/actions.ts`, which names them by
  the generated `*QueryKey` with only their path, so that a key matches every query of the read, and a renamed
  operation fails the type check. Its refusal shows with `MutationError` (`components/errors/mutation-error.tsx`),
  in the words of `refusalOf` (`domain/problem.ts`). No optimistic update of the queue: the API
  answers `202` and the queue decides at its next reconciliation; the page shows the request as sent until the
  stream signals the change.
- An error is the `Problem` of the API: the page decides on its `code`, never on `detail` or on the status alone.

## Loading, errors and actions

- A page shows its skeleton (`Skeleton` of `@cihub/ui`, the shapes of the mockups v5) when its reads take more than 200
  ms, then for 500 ms at least, so that it never flashes (`defaultPendingMs`, `defaultPendingMinMs`). The frame
  stays: only the page waits. While the frame of an organization loads, it shows the skeleton of the page that the
  address opens. A part that waits on its own (a tab, the run of a PR) shows the skeleton of that part.
- A failed read keeps what the page can still show (the frame, the head of a queue, the drawer) and says, in a card,
  what the server answered, with "Try again" (`components/errors/server-error.tsx`). A `401` sends the person to the
  sign-in and back to the page, once a new read of the person says that the session ended; else it shows as a
  failure, so that the sign-in never loops.
- An action that runs says so in its button: a spinner and what runs ("Submitting"), with `isPending` and
  `pendingText` of `Button`. A refusal of the server shows next to it with `MutationError`; a failure that the page
  words itself ("The check failed") shows with `ErrorAlert` (`components/errors/error-alert.tsx`).
- The skeleton of a page sits in its `loading.tsx`, which never imports `page.tsx`, `layout.tsx` or `error.tsx`:
  `autoCodeSplitting` keeps a `pendingComponent` in the entry chunk, and a skeleton imported from the file of its
  page would bring the whole page into it.
- `mise run console:shots` captures these states too: a view whose read never answers shows the skeleton.
- What an action did shows for a few seconds with `useTimedValue` (`hooks/use-timed-value.ts`), never with an
  effect and a timer of its own.

## Reading order of the code

The code is read by a person first: the lines that do one thing sit together, as one block, and one blank line
separates two blocks. A component or a hook keeps this order, each block only when it has it:

1. what the route gives: `getRouteApi(...).useParams()`, `useSearch()`, and the values built from them (`path`);
2. the reads and the actions of the server: `useSuspenseQuery`, `useMutation`, the hooks that read;
3. the local state: `useState`, `useTimedValue`;
4. the values derived from the blocks above, and the state that they set during the render;
5. the effects, if any;
6. the render, after one blank line.

A guard clause is followed by one blank line. A block that grows past what the eye takes in at once moves to a
hook of the page (`use-*.ts`) or to a function of `domain/`. A loader follows its three steps in the same way.

## Animations

- The animations follow the mockups v5: the drawer of a PR slides in from the right and its scrim fades in; a page
  fades in as it opens; a menu fades in and out; a modal fades in; a notice slides in and out. Their timings are
  the `transitions` of `@cihub/ui` (the `ease` of CSS, 120 to 200 ms); a component never writes its own.
- An element that leaves animates out with `<AnimatePresence>` when its component owns its state (a menu, a
  notice). The drawer is a route: Close, Escape and a click beside it slide it out first, then go back to the page.
- A person who asks for less motion gets no animation at all, fades included (`skipAnimations` of
  `MotionProvider`); the tests and `mise run console:shots` ask for it too, so that a capture never depends on a frame.
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

- The look comes from `@cihub/theme`: its `tokens.css` holds the colors of the mockups v32 in two themes
  (`data-theme`), and its `tailwind.css` the Tailwind theme that makes them utilities. `src/styles/globals.css`
  imports both, then adds the utilities and the base styles of the console only. The theme holds these tokens only: a color, a radius, a shadow, a
  font or a size of the Tailwind palette does not exist, so `bg-white` or `text-gray-500` is a build without the
  class.
- Colors by role, never by hue: `bg-card`, `text-muted-foreground`, `border-destructive-border`, and the `info`,
  `warning`, `destructive`, `success` tones with their `-muted` (background) and `-border`. The dark theme comes
  from the tokens: a component names a theme (`dark:`, which follows `data-theme`) only where no token can tell
  the two apart.
- No arbitrary value (`text-[13px]`, `bg-[#fff]`, `rounded-[5px]`) for a color, a size of text, a radius or a
  shadow: use a token of `@cihub/theme`, or add one. An arbitrary value stays for a layout that only one component
  has (`grid-cols-[auto_1fr]`).
- A class goes on the element that it styles, never on a parent through a descendant selector (`[&_p]:`), but for
  the icons that a component sizes in its own slot (`[&>svg]:size-4`): a part that repeats is a component
  (`DrawerSection`, `Setting`). A repeated look is a component of `@cihub/ui`, not a list
  of classes copied from page to page.
- Every action works with the keyboard; `Escape` closes a menu, a drawer or a modal. A page has its loading, empty
  and error states.

## Commands

`mise run lint` and `mise run check` cover the dashboard: type check, ESLint (`typescript-eslint`,
`react-hooks`, `@tanstack/eslint-plugin-query`, `@tanstack/eslint-plugin-router`), `oxfmt --check`, Vitest.
`mise run console:fmt` formats the dashboard; the pre-commit hook (`.lefthook.toml`) formats the files that a
commit stages, as `cargo fmt` does for the Rust code.
`mise run console:dev` serves it with Vite in front of a `cihub-server start` (`CIHUB_URL`, `http://localhost:8080`);
`mise run console:build` builds `dist/`, which the next build of `cihub-web` embeds; `mise run console:generate`
writes `schema/openapi/v1.json` from the code (`cargo run -p cihub-server --example openapi`) and the client in `src/client`;
`mise run console:e2e` runs the browser tests of `e2e/` (Playwright) against `cargo run -p cihub-server --example e2e`, a
server on the simulator with a fresh database, which runs the queues, submits PRs of `acme/web` at its start,
passes each test commit of `acme/api` but those that test its #3, comments `/cihub merge` on a PR at `/sim/merge`, and gives the last
comment of a PR at `/sim/comment`.

`mise run console:shots` captures the views of `e2e/visual/views.ts` (each page, in the states that the simulator
cannot show, in both themes and in a narrow window) from a build of the dashboard, against the API of
`e2e/visual/api.ts` and a fixed clock, into `test-reports/console-visual/head`, which git ignores: two captures of the
same build show the same thing, but the runner of the CI can draw an anti-aliased edge with a few different pixels.
`mise run console:shots-diff [REF]` (`origin/main` by default) builds REF in a worktree, captures
both, and writes into `test-reports/console-visual/diff` the captures of each view that differs, is new or is gone
(`before/`, `after/`), its changed pixels (`changes/`, as `pixelmatch` finds them: it ignores the anti-aliased
edges and the changes that the eye does not see), `report.md`, and `index.html`, which shows each of these
views before and after: side by side, split, blend, or its changed pixels, zoomed on the change. A view that did not
change is only counted. On a pull request that changes `apps/console/` or `packages/`, the job `Console Visual` of the CI runs it against
the base, writes `report.md` into the summary of the run, and uploads `diff/` as the artifact `dashboard-visual`:
download it and open `index.html`. It gates nothing. Its viewer (`e2e/visual/viewer/`) uses the design system and the components of
`src/`, built as one script, as a page opened from the disk runs no module. The fixtures use the types of `src/client`: a change of the API that they no longer
follow fails the type check. A new page or a new state adds its view to `views.ts`.

Specs: `specs/API.md`, `specs/SECURITY.md`, `specs/LOTS.md` (lots 10a to 10d, 11a).
