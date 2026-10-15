// The views that a change of the dashboard can move: each page, in the states that the simulator cannot show
// (a paused queue, a failed run, a refusal), its menus and its dialogs.
import type { Page } from "@playwright/test";

import { type Answer, type ApiRequest, type Person, answerOf, problem } from "./api";

export type Theme = "light" | "dark";

export interface Viewport {
  width: number;
  height: number;
}

export interface View {
  name: string;
  url: string;
  /** A text that shows once the view has read its data, or `skeleton` for a view that loads. */
  ready: string;
  person?: Person;
  themes?: Theme[];
  viewports?: Viewport[];
  /** What the person does before the capture: opens a menu, submits a form. */
  act?: (page: Page) => Promise<void>;
  /** An answer of the API that differs from the fixtures; `null` keeps the one of the fixtures. */
  answer?: (request: ApiRequest) => Answer | null;
  /** A read that never answers: the view shows how the page waits for it. */
  hold?: (request: ApiRequest) => boolean;
  /** The stream of the organization never opens: the graph shows as not live. */
  isStreamClosed?: boolean;
  /** The status of `/auth/sign-out`. */
  signOutStatus?: number;
}

export const WIDE: Viewport = { width: 1280, height: 800 };
const NARROW: Viewport = { width: 700, height: 900 };
const SIGN_IN_NARROW: Viewport = { width: 850, height: 800 };
const BOTH: Theme[] = ["light", "dark"];

const QUEUE_PAGE = "/orgs/acme/repos/web/queues/main";
const QUEUE_READY = "Bump the version";
const DRAWER = `${QUEUE_PAGE}/prs/118`;

async function submitPr(page: Page, number: string): Promise<void> {
  await page.getByRole("button", { name: "Submit a pull request" }).click();
  await page.getByRole("dialog").getByRole("textbox").fill(number);
  await page.getByRole("dialog").getByRole("button", { name: "Submit" }).click();
}

async function pauseQueue(page: Page): Promise<void> {
  await page.getByRole("button", { name: "Pause" }).click();
  await page.getByRole("dialog").getByRole("button", { name: "Pause" }).click();
}

export const VIEWS: View[] = [
  {
    name: "sign-in",
    url: "/sign-in",
    person: "nobody",
    ready: "Continue with GitHub",
    themes: BOTH,
    viewports: [WIDE, SIGN_IN_NARROW],
  },
  { name: "sign-in-failed", url: "/sign-in?error=failed", person: "nobody", ready: "The sign-in failed" },
  { name: "sign-in-no-org", url: "/sign-in", person: "grace", ready: "No organization yet", themes: BOTH },
  {
    name: "device",
    url: "/device",
    ready: "Sign in the command line",
    themes: BOTH,
    act: async (page) => {
      await page.getByRole("textbox", { name: "Letter 1 of 8" }).fill("WDJBMJHT");
    },
  },
  { name: "device-typed", url: "/device", ready: "Type the code" },
  {
    name: "device-approved",
    url: "/device",
    ready: "Sign in the command line",
    act: async (page) => {
      await page.getByRole("textbox", { name: "Letter 1 of 8" }).fill("WDJBMJHT");
      await page.getByRole("button", { name: "Approve as ada" }).click();
      await page.getByText("Command line signed in").waitFor();
    },
  },
  {
    name: "device-unknown-code",
    url: "/device",
    ready: "Sign in the command line",
    answer: ({ method }) => (method === "POST" ? [404, problem(404, "device_code_not_found")] : null),
    act: async (page) => {
      await page.getByRole("textbox", { name: "Letter 1 of 8" }).fill("WDJBMJHT");
      await page.getByRole("button", { name: "Approve as ada" }).click();
      await page.getByText("No command line waits for this code").waitFor();
    },
  },
  {
    name: "org-overview",
    url: "/orgs/acme",
    ready: "Your pull requests",
    themes: BOTH,
    viewports: [WIDE, NARROW],
  },
  {
    name: "org-overview-filtered",
    url: "/orgs/acme",
    ready: "Your pull requests",
    act: (page) => page.getByRole("searchbox").fill("zzz"),
  },
  { name: "org-empty", url: "/orgs/acme-labs", ready: "No queue yet", themes: BOTH },
  { name: "repo-overview", url: "/orgs/acme/repos/web", ready: "release" },
  { name: "queue-live", url: QUEUE_PAGE, ready: QUEUE_READY, themes: BOTH, viewports: [WIDE, NARROW] },
  { name: "queue-not-live", url: QUEUE_PAGE, ready: QUEUE_READY, isStreamClosed: true },
  { name: "queue-merged", url: `${QUEUE_PAGE}?tab=merged`, ready: "Add the audit log" },
  { name: "queue-ended", url: `${QUEUE_PAGE}?tab=ended`, ready: "Remove the old cache", themes: BOTH },
  { name: "queue-banners", url: "/orgs/acme/repos/web/queues/release", ready: "Paused by", themes: BOTH },
  { name: "queue-access-lost", url: "/orgs/acme/repos/mobile/queues/main", ready: "lost its access" },
  { name: "queue-draining", url: "/orgs/acme/repos/api/queues/main", ready: "Draining." },
  {
    name: "queue-quiet",
    url: "/orgs/acme/repos/sdk/queues/main",
    ready: "No pull request in the queue",
    themes: BOTH,
  },
  {
    name: "queue-quiet-copied",
    url: "/orgs/acme/repos/sdk/queues/main",
    ready: "No pull request in the queue",
    act: async (page) => {
      // The views are served over plain HTTP, which has no clipboard.
      await page.evaluate(() => {
        Object.defineProperty(navigator, "clipboard", { value: { writeText: () => Promise.resolve() } });
      });
      await page.getByRole("button", { name: "Copy" }).click();
      await page.getByText("Copied /cihub merge to the clipboard.").waitFor();
    },
  },
  // Nothing in the queue, but a merge in the last 24 hours: the tabs stay, and the tab of the queue is empty.
  {
    name: "queue-empty-tab",
    url: "/orgs/acme/repos/infra/queues/main",
    ready: "No pull request in the queue",
  },
  {
    name: "queue-health",
    url: `${QUEUE_PAGE}?pane=health`,
    ready: "looks flaky",
    themes: BOTH,
    viewports: [WIDE, NARROW],
  },
  {
    name: "queue-health-custom",
    url: `${QUEUE_PAGE}?pane=health&from=2026-09-14T00:00:00.000Z&to=2026-10-12T00:00:00.000Z`,
    ready: "looks flaky",
  },
  {
    name: "queue-health-calendar",
    url: `${QUEUE_PAGE}?pane=health`,
    ready: "looks flaky",
    themes: BOTH,
    viewports: [WIDE, NARROW],
    act: async (page) => {
      await page.getByRole("button", { name: "Custom" }).click();
      await page.getByRole("button", { name: "Last month" }).click();
      await page.getByText("compared with").waitFor();
    },
  },
  {
    name: "queue-health-quiet",
    url: "/orgs/acme/repos/sdk/queues/main?pane=health",
    ready: "Show the last 7 days",
  },
  {
    name: "queue-health-loading",
    url: `${QUEUE_PAGE}?pane=health`,
    ready: "skeleton",
    hold: ({ path }) => path.endsWith("/health"),
  },
  { name: "drawer-testing", url: DRAWER, ready: "History", themes: BOTH, viewports: [WIDE, NARROW] },
  { name: "drawer-not-ready", url: `${QUEUE_PAGE}/prs/133`, ready: "Why it waits" },
  { name: "drawer-merged", url: `${QUEUE_PAGE}/prs/110`, ready: "Outcome" },
  { name: "drawer-not-found", url: `${QUEUE_PAGE}/prs/999`, ready: "is not in this queue" },
  {
    name: "drawer-restart-sent",
    url: DRAWER,
    ready: "History",
    act: async (page) => {
      await page.getByRole("button", { name: "Restart tests" }).click();
      await page.getByText("Restart sent").waitFor();
    },
  },
  {
    name: "modal-submit-invalid",
    url: QUEUE_PAGE,
    ready: QUEUE_READY,
    themes: BOTH,
    act: async (page) => {
      await page.getByRole("button", { name: "Submit a pull request" }).click();
      await page.getByRole("dialog").getByRole("button", { name: "Submit" }).click();
    },
  },
  {
    name: "modal-submit-refused",
    url: QUEUE_PAGE,
    ready: QUEUE_READY,
    answer: ({ method }) =>
      method === "POST"
        ? [409, problem(409, "pr_already_queued", "the PR is already in the queue, `testing`")]
        : null,
    act: async (page) => {
      await submitPr(page, "118");
      await page.getByRole("dialog").getByRole("alert").waitFor();
    },
  },
  {
    name: "submit-sent",
    url: QUEUE_PAGE,
    ready: QUEUE_READY,
    act: async (page) => {
      await submitPr(page, "200");
      await page.getByText("#200 sent").waitFor();
    },
  },
  {
    name: "modal-pause",
    url: QUEUE_PAGE,
    ready: QUEUE_READY,
    themes: BOTH,
    act: (page) => page.getByRole("button", { name: "Pause" }).click(),
  },
  {
    name: "queue-pausing",
    url: QUEUE_PAGE,
    ready: QUEUE_READY,
    act: async (page) => {
      await pauseQueue(page);
      await page.getByText("Pausing the queue…").waitFor();
    },
  },
  {
    name: "queue-paused-now",
    url: QUEUE_PAGE,
    ready: QUEUE_READY,
    act: async (page) => {
      // The queue takes the request at once: its next read is paused. A read before the request, as the one
      // that the stream asks when it opens, still finds it running, so the Pause button stays.
      let isRequested = false;
      await page.route("**/api/v1/orgs/acme/repos/web/queues/main", (route) => {
        if (route.request().method() !== "GET") {
          isRequested = true;
          return route.fallback();
        }
        if (!isRequested) {
          return route.fallback();
        }
        const [status, body] = answerOf(
          { path: "/orgs/acme/repos/web/queues/main", query: new URLSearchParams(), method: "GET" },
          "ada",
        );
        return route.fulfill({
          status,
          contentType: "application/json",
          body: JSON.stringify({ ...body, state: "paused" }),
        });
      });
      await pauseQueue(page);
      await page.getByText("The queue is paused.").waitFor();
    },
  },
  {
    name: "modal-cancel",
    url: DRAWER,
    ready: "History",
    act: (page) => page.getByRole("button", { name: "Cancel", exact: true }).click(),
  },
  {
    name: "menu-org",
    url: QUEUE_PAGE,
    ready: QUEUE_READY,
    act: (page) => page.getByRole("button", { name: /^Organization:/ }).click(),
  },
  {
    name: "menu-repo",
    url: QUEUE_PAGE,
    ready: QUEUE_READY,
    act: (page) => page.getByRole("button", { name: /^Repository:/ }).click(),
  },
  {
    name: "menu-queue",
    url: QUEUE_PAGE,
    ready: QUEUE_READY,
    themes: BOTH,
    act: (page) => page.getByRole("button", { name: /^Queue:/ }).click(),
  },
  {
    name: "menu-person",
    url: "/orgs/acme",
    ready: "Your pull requests",
    act: (page) => page.getByRole("button", { name: /^Your account:/ }).click(),
  },
  {
    name: "sign-out-failed",
    url: "/orgs/acme",
    ready: "Your pull requests",
    signOutStatus: 500,
    act: async (page) => {
      await page.getByRole("button", { name: /^Your account:/ }).click();
      await page.getByRole("button", { name: "Sign out" }).click();
      await page.getByRole("alert").waitFor();
    },
  },
  { name: "org-lost", url: "/orgs/globex", ready: "You have no access", themes: BOTH },
  { name: "not-found", url: "/orgs/acme/repos/nothing", ready: "Page not found" },
  // Loading: a read that never answers keeps the skeleton of the page.
  {
    name: "org-loading",
    url: "/orgs/acme",
    ready: "skeleton",
    themes: BOTH,
    hold: ({ path }) => path === "/orgs/acme/queues",
  },
  {
    name: "queue-first-loading",
    url: QUEUE_PAGE,
    ready: "skeleton",
    hold: ({ path }) => path === "/orgs/acme/queues",
  },
  {
    name: "queue-loading",
    url: QUEUE_PAGE,
    ready: "skeleton",
    themes: BOTH,
    viewports: [WIDE, NARROW],
    hold: ({ path }) => path.endsWith("/graph"),
  },
  { name: "drawer-loading", url: DRAWER, ready: "skeleton", hold: ({ path }) => path.endsWith("/events") },
  {
    name: "run-loading",
    url: DRAWER,
    ready: "skeleton",
    hold: ({ path }) => path.startsWith("/orgs/acme/runs/"),
  },
  {
    name: "tab-loading",
    url: QUEUE_PAGE,
    ready: QUEUE_READY,
    hold: ({ query }) => query.get("state") === "merged" && query.get("repo") === "web",
    act: async (page) => {
      await page.getByRole("link", { name: /^Merged/ }).click();
      await page.waitForTimeout(800);
    },
  },
  {
    name: "submitting",
    url: QUEUE_PAGE,
    ready: QUEUE_READY,
    hold: ({ method }) => method === "POST",
    act: async (page) => {
      await submitPr(page, "200");
      await page.getByText("Submitting").waitFor();
    },
  },
  // Errors: the page keeps its head, the drawer its frame.
  {
    name: "drawer-error",
    url: DRAWER,
    ready: "could not load this page",
    answer: ({ path }) => (path === "/orgs/acme/repos/web/prs/118" ? [503, problem(503, "internal")] : null),
  },
  {
    name: "org-error",
    url: "/orgs/acme",
    ready: "could not load this page",
    answer: ({ path }) => (path === "/orgs/acme/queues" ? [503, problem(503, "internal")] : null),
  },
  {
    name: "server-error",
    url: QUEUE_PAGE,
    ready: "could not load this page",
    themes: BOTH,
    answer: ({ path }) => (path.endsWith("/graph") ? [503, problem(503, "internal")] : null),
  },
];
