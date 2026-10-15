// Captures each view of `views.ts` from a build of the dashboard, with the API of `api.ts` and a fixed start time:
// `mise run web:shots`. No server: the page reads the files of the build and the answers of the fixtures.
import { existsSync, readFileSync, statSync } from "node:fs";
import { extname, join, resolve } from "node:path";

import { type BrowserContext, type Page, type Route, test } from "@playwright/test";

import { NOW, answerOf } from "./api";
import { type Theme, VIEWS, type View, type Viewport, WIDE } from "./views";

const BASE_VIEW_TIMEOUT_MS = 5_000;
const BUILD = resolve(process.env.CIHUB_VISUAL_BUILD ?? "dist");
const IS_BASE = process.env.CIHUB_VISUAL_LABEL === "base";
const ORIGIN = "http://cihub.test";
const OUT = resolve(process.env.CIHUB_VISUAL_OUT ?? "e2e/__screenshots__/head");

const CONTENT_TYPES: Record<string, string> = {
  ".html": "text/html",
  ".js": "text/javascript",
  ".css": "text/css",
  ".svg": "image/svg+xml",
};

/** A file of the build, or `index.html` for an address of the router. */
function fileOf(pathname: string): string {
  const file = join(BUILD, pathname);
  return pathname !== "/" && existsSync(file) && statSync(file).isFile() ? file : join(BUILD, "index.html");
}

async function answerRoute(route: Route, view: View): Promise<void> {
  const url = new URL(route.request().url());

  if (url.pathname.startsWith("/api/v1/")) {
    const request = {
      path: url.pathname.slice("/api/v1".length),
      query: url.searchParams,
      method: route.request().method(),
    };
    if (view.hold?.(request)) {
      // Never answers: the page keeps waiting for this read.
      return;
    }
    const [status, body] = view.answer?.(request) ?? answerOf(request, view.person ?? "ada");
    await route.fulfill({
      status,
      contentType: "application/json",
      body: body === null ? "" : JSON.stringify(body),
    });
    return;
  }
  if (url.pathname.startsWith("/auth/")) {
    await route.fulfill({ status: view.signOutStatus ?? 200, body: "" });
    return;
  }

  const file = fileOf(url.pathname);
  await route.fulfill({
    contentType: CONTENT_TYPES[extname(file)] ?? "application/octet-stream",
    body: readFileSync(file),
  });
}

async function openView(context: BrowserContext, view: View, theme: Theme): Promise<Page> {
  const page = await context.newPage();
  // A fixed start that goes on: the router times the skeletons with `Date.now()`, which a frozen clock stops.
  await page.clock.setSystemTime(NOW);
  await page.addInitScript((value) => {
    localStorage.setItem("theme", value);
  }, theme);
  if (!view.isStreamClosed) {
    // A stream that opens and stays quiet: the fixtures already hold the state of the queues.
    await page.addInitScript(() => {
      window.EventSource = class {
        onopen: (() => void) | null = null;

        constructor() {
          setTimeout(() => this.onopen?.());
        }

        addEventListener = () => undefined;
        close = () => undefined;
      } as unknown as typeof EventSource;
    });
  }
  await page.route(`${ORIGIN}/**`, (route) => answerRoute(route, view));

  await page.goto(`${ORIGIN}${view.url}`);
  const ready = view.ready === "skeleton" ? page.locator("[data-slot=skeleton]") : page.getByText(view.ready);
  await ready.first().waitFor();
  await view.act?.(page);
  // The cards and rows slide to their place once the reads settle.
  await page.waitForTimeout(400);
  // A full-page capture draws the sticky header where the page is scrolled: an action can scroll it.
  await page.evaluate(() => {
    window.scrollTo(0, 0);
  });
  return page;
}

function captureName(view: View, theme: Theme, viewport: Viewport): string {
  return `${view.name}-${theme}-${String(viewport.width)}.png`;
}

for (const view of VIEWS) {
  for (const theme of view.themes ?? ["light"]) {
    for (const viewport of view.viewports ?? [WIDE]) {
      const name = captureName(view, theme, viewport);

      test(name, async ({ browser }) => {
        // Less motion, as a person may ask: a capture shows a state, and nothing moves under it.
        const context = await browser.newContext({ viewport, reducedMotion: "reduce" });
        if (IS_BASE) {
          context.setDefaultTimeout(BASE_VIEW_TIMEOUT_MS);
        }
        const page = await openView(context, view, theme).catch((error: unknown) => {
          test.skip(IS_BASE, "The build of the base does not show this view.");
          throw error;
        });

        await page.screenshot({
          path: join(OUT, name),
          fullPage: true,
          animations: "disabled",
          caret: "hide",
        });
        await context.close();
      });
    }
  }
}
