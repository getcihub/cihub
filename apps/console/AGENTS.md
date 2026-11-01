# web

The dashboard: a single-page application in TypeScript, built by Vite and embedded by `cihub-web` in the
`cihub-server` binary. It reads and acts only through the public API; it holds no queue logic and no rule that the
server does not also check.

## Stack

- TypeScript in strict mode (`strict`, `noUncheckedIndexedAccess`); no non-null assertion `!`, no `@ts-ignore`.
  Not `exactOptionalPropertyTypes`: the generated client does not compile with it.
- React, Vite, and the TanStack libraries: Router for the routes, Query for the data of the server, Form for the
  forms. Table only for a table that the client sorts or filters; the server already orders the items.
- The client of the API is generated from `schema/openapi/v1.json` by `@hey-api/openapi-ts`, with its plugin
  `@tanstack/react-query`. The generated files are never edited by hand; a change of the API regenerates them in
  the same change (`mise run console:generate`), and `mise run console:generate-check` fails otherwise. The routes
  outside the API (`/auth/*`, `specs/API.md`) are called with `fetch` in `src/domain/session/session.ts` only.
- Zod 4 for the search parameters and the forms, without adapter (Standard Schema).
- Vitest and Testing Library for the components; Playwright for the browser tests, against a server on the
  simulator.
- Tailwind CSS 4 for every style, with `class-variance-authority` for the variants and `cn()` (`clsx` with
  `tailwind-merge`). No CSS module, no CSS file other than `src/styles/globals.css` and the ones of `@cihub/theme`.
- `motion` for the animations, through `m.*` components under `LazyMotion strict` (`components/motion.tsx` of
  `@cihub/ui`): a `motion.*` component throws.
- oxfmt formats every file (`.oxfmtrc.json`) and sorts the Tailwind classes. Its version is exact: a new one can
  change the format, so its update shows the files that it changes. It skips the generated files.

## Layout

`src/` has five layers, `pages/` > `components/` > `hooks/` > `domain/` > `lib/`: an import goes down, never up, a
page never imports another page, and another folder is imported through `@/`. `eslint.config.js` enforces it.

- `pages/`: one folder per page (see "Pages"). `components/`: the components that two pages or more share, by
  subject, and `frame/`, the frame of the pages of an organization. `hooks/`: the shared hooks (`use-*.ts`).
  `domain/`: one folder per resource of the API, no React. `lib/`: the functions that know nothing of CIHub.
- A folder of `domain/` splits its resource by role: `queries.ts` (one `<thing>Options(...)` per read, over the
  generated `queryOptions`; never a tuple of reads), `actions.ts` (`invalidate<Thing>(queryClient, ...)`, by the
  generated `*QueryKey`), `search.ts` (the Zod schema of the search of a page, its defaults, its tabs),
  `words.ts` (the words and constants that the pages show).
- `domain/problem.ts` holds the failures of the API and the words of a refusal (`failureOf`, `isProblem`,
  `refusalOf`); `domain/read-key.ts` the fields of a query key for a predicate that a `*QueryKey` cannot say.
- `client/` is generated, `styles/` holds the design system, `test/` the helpers of the tests. A file and its test
  sit side by side. A component or a hook that only one page uses sits in the folder of that page; it moves to
  `components/` or `hooks/` when a second page needs it.
- Every file name is in kebab-case. No barrel file (`index.ts`): an import names the file. The packages of
  `packages/` (`packages/ui/AGENTS.md`) are imported by the path of their file (`@cihub/ui/components/button`).

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

- A state that the page shares with others comes from `components/` (`NotFoundPage`, `ServerErrorPage`), and the
  page has no file for it. `src/pages/redirects/` holds the routes that only redirect.
- A zone is a part of the page that a person sees as one (`queue/components/head/`): a component sits in the zone
  that shows it, and a zone imports another one through `@/pages/<page>/components/`. A part that another file
  needs, or a file over about 200 lines, becomes a file of its own.

## Routes

- `src/routes.ts` is the tree of the addresses (`@tanstack/virtual-file-routes`): each address and the `route.tsx`
  of its page. `@tanstack/router-plugin/vite`, placed before `@vitejs/plugin-react`, generates `routeTree.gen.ts`
  from it, with `autoCodeSplitting`. A new page adds its line to `routes.ts` and its folder. The layout of a page is
  the route above it in the tree.
- A `route.tsx` holds the configuration of its route, never a component. A page reads the params and the search of
  its route with `getRouteApi(id)`, never by importing the route.
- An address without a page of its own redirects to the page above it.
- The URL holds the state that a link must keep: the organization, the queue, the open PR, the tab. Every search
  parameter goes through `validateSearch` with a Zod schema.
- A link is a typed `<Link to=…>` or `navigate({ to })`; never a path built from strings.
- The router context carries the `QueryClient`. `src/pages/org/route.tsx` reads the person of the session (the `Me`
  of `src/domain/session/queries.ts`) in its loader, and redirects to the sign-in page on `401`.

## Data of the server

- Every read goes through TanStack Query, through the function of `domain/<resource>/queries.ts`; nothing else
  calls a generated `*Options`, so that a read changes in one place. Never a `fetch` by hand, never a copy of the
  data of the server in `useState`, a context or a store.
- A loader starts its reads with `queryClient.query(...)` (`ensureQueryData` is deprecated), in three steps
  (`src/pages/queue/route.tsx`): it decides `notFound()` or `redirect()` on the `code` of a `Problem`; it starts,
  without awaiting them, the reads of a part that waits in a `<Suspense>` of its own; then it awaits the reads
  that the whole page shows together, with `Promise.allSettled`. The component reads them with `useSuspenseQuery`.
  No request waterfall.
- A link starts the reads of its page when the pointer rests on it (`defaultPreload: "intent"`,
  `defaultPreloadStaleTime: 0`).
- The stream `/orgs/{org}/stream` (`src/domain/queue/live.ts`, `src/pages/org/use-queue-news.ts`) invalidates the
  query keys of the queue that changed, gathered over half a second, and every read of the organization each time
  it opens. No polling, with two exceptions: the Health of a queue reads again every 30 seconds while it is open,
  because its facts come a few seconds after the change; a page that waits for a read of the SCM reads again every
  2 seconds, for 60 seconds at most (`readAgainWhile` of `src/domain/setup/read-again.ts`).
- A mutation is `useMutation({ ...<operation>Mutation(), onSettled: () => invalidate<Thing>(queryClient, ...) })`.
  The key names only the path, so that it matches every query of the read, and a renamed operation fails the type
  check. Its refusal shows with `MutationError`, in the words of `refusalOf`. No optimistic update of the queue: the
  API answers `202` and the queue decides at its next reconciliation; the page shows the request as sent until the
  stream signals the change.
- An error is the `Problem` of the API: the page decides on its `code`, never on `detail` or on the status alone.

## Loading, errors and actions

- A page shows its skeleton (`Skeleton` of `@cihub/ui`) when its reads take more than 200 ms, then for 500 ms at
  least, so that it never flashes (`defaultPendingMs`, `defaultPendingMinMs`). The frame stays: only the page
  waits. A part that waits on its own shows the skeleton of that part.
- A failed read keeps what the page can still show and says, in a card, what the server answered, with "Try again"
  (`components/errors/server-error.tsx`). A `401` sends the person to the sign-in and back, once a new read of the
  person says that the session ended; else it shows as a failure, so that the sign-in never loops.
- An action that runs says so in its button (`isPending` and `pendingText` of `Button`). A refusal of the server
  shows next to it with `MutationError`; a failure that the page words itself shows with `ErrorAlert`.
- `loading.tsx` never imports `page.tsx`, `layout.tsx` or `error.tsx`: `autoCodeSplitting` keeps a
  `pendingComponent` in the entry chunk, and an import from its page would bring the whole page into it.
- What an action did shows for a few seconds with `useTimedValue` (`hooks/use-timed-value.ts`), never with an
  effect and a timer of its own.

## Reading order of the code

A component or a hook keeps this order, one blank line between two blocks, each block only when it has it:

1. what the route gives: `getRouteApi(...).useParams()`, `useSearch()`, and the values built from them;
2. the reads and the actions of the server: `useSuspenseQuery`, `useMutation`, the hooks that read;
3. the local state: `useState`, `useTimedValue`;
4. the values derived from the blocks above, and the state that they set during the render;
5. the effects, if any;
6. the render.

A block that grows past what the eye takes in at once moves to a hook of the page or to a function of `domain/`.

## Animations

- The animations follow the mockups v5. Their timings are the `transitions` of `@cihub/ui` (120 to 200 ms); a
  component never writes its own.
- An element that leaves animates out with `<AnimatePresence>` when its component owns its state. The drawer is a
  route: Close, Escape and a click beside it slide it out first, then go back to the page.
- A person who asks for less motion gets no animation at all (`skipAnimations` of `MotionProvider`); the tests and
  the captures ask for it too, so that a capture never depends on a frame.
- The features of `motion` load with the first page: a page and the drawer start hidden until they animate, and a
  later chunk would leave them hidden.

## Vocabulary and rights

- The page shows `state_description` and the descriptions that the API gives; it decides on the codes
  (`state_reason`, `kind`, `code`). It never writes its own sentence for a reason of the closed vocabulary.
- The role of the person only hides or disables a button; the server refuses what the role does not allow.
- Every text is in English, with the words of `specs/DOMAIN.md`. The texts may name GitHub, as the mockups do
  ("Continue with GitHub"), an exception to the neutral vocabulary that the project lead accepted.

## Interface

- The look comes from `@cihub/theme` (`tokens.css`, the colors of the mockups v32 in two themes, and
  `tailwind.css`); `src/styles/globals.css` imports both, then adds the utilities and base styles of the console
  only. A color, radius, shadow, font or size of the Tailwind
  palette does not exist, so `bg-white` is a build without the class.
- Colors by role, never by hue: `bg-card`, `text-muted-foreground`, and the `info`, `warning`, `destructive`,
  `success` tones with their `-muted` and `-border`. A component names a theme (`dark:`) only where no token can
  tell the two apart.
- No arbitrary value (`text-[13px]`, `bg-[#fff]`) for a color, a size of text, a radius or a shadow: use a token,
  or add one. An arbitrary value stays for a layout that only one component has (`grid-cols-[auto_1fr]`).
- A class goes on the element that it styles, never on a parent through a descendant selector (`[&_p]:`), but for
  the icons that a component sizes in its own slot (`[&>svg]:size-4`). A repeated look is a component of
  `@cihub/ui`, not a list of classes copied from page to page.
- Every action works with the keyboard; `Escape` closes a menu, a drawer or a modal. A page has its loading, empty
  and error states.

## Commands

`mise run check` covers the dashboard (type check, ESLint, `oxfmt --check`, Vitest); `mise run console:fmt`
formats it. `mise tasks` describes the others: `console:dev`, `console:build`, `console:generate`, `console:e2e`
(Playwright against `cargo run -p cihub-server --example e2e`, a server on the simulator).

`mise run console:shots` captures the views of `e2e/visual/views.ts` with a mocked API and a fixed clock;
`mise run console:shots-diff [REF]` compares them with a git ref (`e2e/visual/shots.sh` says how). A new page or a
new state adds its view to `views.ts`. The job `Console Visual` of the CI runs the comparison on a pull request
that changes `apps/console/` or `packages/`, and gates nothing.

Specs: `specs/API.md`, `specs/SECURITY.md`, `specs/LOTS.md` (lots 10a to 10d, 11a).
