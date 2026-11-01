# website

The site of `https://cihub.io`: the page that presents CIHub, and its documentation. Next.js and Fumadocs, deployed
on Vercel with `apps/website/` as the Root Directory. It holds no queue logic and calls no instance: it describes CIHub.

## For agents first

An agent that discovers CIHub, or helps a person set up a queue, finds on this site everything a person finds, in
Markdown. A change keeps these URLs working; `mise run website:test` checks them.

| URL                                                  | Served by                                                       |
| ---------------------------------------------------- | --------------------------------------------------------------- |
| `/llms.txt`                                          | `src/app/llms.txt/route.ts`: the index                          |
| `/llms-full.txt`                                     | `src/app/llms-full.txt/route.ts`: every page                    |
| `/docs/<page>.md`                                    | `src/proxy.ts`, then `src/app/llms.mdx/`                        |
| `/docs/<page>` with `Accept: text/markdown`, and `/` | `src/proxy.ts`: the same Markdown, and `/llms.txt`              |
| `/schema/config/v1.json`                             | a copy of `schema/config/v1.json`                               |
| `/openapi/v1.json`                                   | a copy of `schema/openapi/v1.json`, without the relative server |

- The build copies the two files of the repository into `public/` (`scripts/publish-machine-files.ts`, which
  git ignores): the repository keeps a single source for each. It also writes `generated/error-codes.md` from the
  examples of the OpenAPI document, which `content/docs/api/errors.mdx` includes: a section per error code, whose
  anchor is the `type` of its problem.
- A Markdown URL that does not exist answers 404 with the list of the pages that do, so that an agent corrects
  itself.
- `next.config.ts` redirects the paths that an agent guesses (`/install`, `/quickstart`) to the real page.

## Analytics

`src/instrumentation-client.ts` sends the visits of a browser to PostHog (EU), in cookieless mode: no cookie, no
storage, no consent banner. Only the production deployment of Vercel sets `NEXT_PUBLIC_POSTHOG_PROJECT_TOKEN`;
without it, the site sends nothing. The events go through `/ingest`, which `next.config.ts` rewrites to PostHog.
The project of PostHog must enable cookieless mode in its settings, or it drops these events.

## Writing a page

- A page is an `.mdx` file of `content/docs/`, listed in `meta.json`, with a `title` and a `description`. Both go
  into `llms.txt`, the `<title>` and the description for search engines: the description says what the page is
  for in one sentence.
- The title is the only H1; `src/lib/remark-drop-title.ts` drops any other. The headings go H2 then H3, and say
  their subject ("Protect the target branch", not "Overview").
- A page answers one task, lists what it needs first, and gives complete commands to copy.
- A text that the repository already holds is included, not copied: `<include>../../../../INSTALL.md</include>`. Its
  links are absolute, so that they work on GitHub and here.
- The colors come from `@cihub/theme`, the logo and the icons from `@cihub/icons` (`packages/ui/AGENTS.md`):
  `src/app/global.css` imports `tokens.css` and gives the tokens to the colors of Fumadocs; it does not import
  `tailwind.css`, whose scales would replace the ones that Fumadocs uses. The website does not use `@cihub/ui`.
- Same vocabulary as the rest of the repository: a reason, a state or a setting is named as `specs/DOMAIN.md` and
  `specs/CONFIGURATION.md` name it.

## Commands

`mise run website:dev` serves the site, `mise run website:lint` type checks, lints and checks the format,
`mise run website:test` builds the site and checks what an agent reads on it.

<!-- BEGIN:nextjs-agent-rules -->

## This is NOT the Next.js you know

This version has breaking changes — APIs, conventions, and file structure may all differ from your training data. Read the relevant guide in `node_modules/next/dist/docs/` (resolved from this file's directory; in monorepos the `next` package may not be visible from the repo root) before writing any code. Heed deprecation notices.

This block is written and re-added by `next dev` — verify at `node_modules/next/dist/server/lib/generate-agent-files.js`. Removing it from a diff only re-creates the uncommitted change; committing it with your work keeps the tree clean.

<!-- END:nextjs-agent-rules -->
