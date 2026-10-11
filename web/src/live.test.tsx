import { screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { type Answers, renderApp } from "./test/app";
import type { OrgPr } from "./client/types.gen";
import { ada, detailOf, itemOf, noGraph, queueOf } from "./test/fixtures";

/** The streams that the page opened, as the browser would. */
class FakeEventSource {
  static opened: FakeEventSource[] = [];
  onopen: (() => void) | null = null;
  isClosed = false;
  readonly listeners = new Map<string, (event: { data: string }) => void>();

  constructor(readonly url: string) {
    FakeEventSource.opened.push(this);
  }

  addEventListener(type: string, listener: (event: { data: string }) => void) {
    this.listeners.set(type, listener);
  }

  close() {
    this.isClosed = true;
  }

  emit(type: string, data: unknown) {
    this.listeners.get(type)?.({ data: JSON.stringify(data) });
  }
}

const QUEUE = "/api/v1/orgs/acme/repos/web/queues/main";
const web = queueOf("web", "main");
const api = queueOf("api", "main");
const answers: Answers = {
  "/api/v1/me": [200, ada],
  "/api/v1/orgs/acme/prs": [200, { prs: [] }],
  "/api/v1/orgs/acme/queues": [200, { queues: [api, web] }],
  [QUEUE]: [200, detailOf(web)],
  [`${QUEUE}/graph`]: [200, noGraph],
  [`${QUEUE}/config-check`]: [200, { findings: [], checked_at: null }],
  [`${QUEUE}/prs`]: [200, { items: [] }],
};

/** How many times the page read `path`. */
function readsOf(path: string): number {
  return vi.mocked(fetch).mock.calls.filter(([input]) => new URL((input as Request).url).pathname === path)
    .length;
}

afterEach(() => {
  vi.unstubAllGlobals();
  FakeEventSource.opened = [];
});

describe("the stream of an organization", () => {
  it("reads again the queue that changed, and everything after a new stream", async () => {
    vi.stubGlobal("EventSource", FakeEventSource);
    renderApp("/orgs/acme/repos/web/queues/main", answers);
    await screen.findByRole("heading", { level: 1, name: "main" });

    const [stream] = FakeEventSource.opened;
    const before = readsOf(QUEUE);

    expect(stream?.url).toBe("http://localhost/api/v1/orgs/acme/stream");

    stream?.onopen?.();
    stream?.emit("queue_changed", { queue_id: api.queue.id, seq: 3 });
    await waitFor(() => {
      expect(readsOf("/api/v1/orgs/acme/queues")).toBeGreaterThan(1);
    });

    expect(readsOf(QUEUE)).toBe(before);

    stream?.emit("queue_changed", { queue_id: web.queue.id, seq: 4 });
    await waitFor(() => {
      expect(readsOf(QUEUE)).toBe(before + 1);
    });

    stream?.onopen?.();
    await waitFor(() => {
      expect(readsOf(`${QUEUE}/config-check`)).toBeGreaterThan(1);
    });
  });

  it("tells the person that one of their PRs moved", async () => {
    const prOf = (state: string): OrgPr => ({
      repo: "web",
      queue: "main",
      item: itemOf(124, state, { author: "ada", title: "Fix the login redirect" }),
      ahead: [],
      merges_in_seconds: null,
    });
    vi.stubGlobal("EventSource", FakeEventSource);
    renderApp("/orgs/acme/repos/web/queues/main", {
      ...answers,
      "/api/v1/orgs/acme/prs": [
        [200, { prs: [prOf("pending")] }],
        [200, { prs: [prOf("testing")] }],
      ],
    });
    await screen.findByText("1st in line");

    FakeEventSource.opened[0]?.emit("queue_changed", { queue_id: web.queue.id, seq: 5 });

    expect(await screen.findByText("#124 Fix the login redirect: testing")).toBeInTheDocument();
  });
});
