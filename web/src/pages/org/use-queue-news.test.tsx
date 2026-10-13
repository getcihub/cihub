import { screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { ItemState } from "@/client/types.gen";
import { type Answers, renderApp } from "@/test/app";
import {
  ada,
  answersOfMe,
  answersOfMine,
  detailOf,
  itemOf,
  liveOf,
  noGraph,
  pageOf,
  queueOf,
} from "@/test/fixtures";

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

  emit(type: string, data: object) {
    this.listeners.get(type)?.({ data: JSON.stringify(data) });
  }
}

const QUEUE = "/api/v1/orgs/acme/repos/web/queues/main";
const web = queueOf("web", "main");
const api = queueOf("api", "main");
const answers: Answers = {
  ...answersOfMe(ada),
  ...answersOfMine("acme"),
  "/api/v1/orgs/acme/queues": [200, pageOf("queues", [api, web])],
  [QUEUE]: [200, detailOf(web)],
  [`${QUEUE}/graph`]: [200, noGraph],
  [liveOf()]: [200, pageOf("prs", [])],
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
  it("reads everything when the stream opens, then only the queue that changed", async () => {
    vi.stubGlobal("EventSource", FakeEventSource);
    renderApp("/orgs/acme/repos/web/queues/main", answers);
    await screen.findByRole("heading", { level: 1, name: "main" });

    const [stream] = FakeEventSource.opened;
    const graphs = readsOf(`${QUEUE}/graph`);

    expect(stream?.url).toBe("http://localhost/api/v1/orgs/acme/stream");

    stream?.onopen?.();
    await waitFor(() => {
      expect(readsOf(`${QUEUE}/graph`)).toBe(graphs + 1);
    });

    const queues = readsOf("/api/v1/orgs/acme/queues");
    const before = readsOf(QUEUE);
    stream?.emit("queue_changed", { queue_id: api.id, seq: 3 });
    await waitFor(() => {
      expect(readsOf("/api/v1/orgs/acme/queues")).toBe(queues + 1);
    });

    expect(readsOf(QUEUE)).toBe(before);

    stream?.emit("queue_changed", { queue_id: web.id, seq: 4 });
    stream?.emit("queue_changed", { queue_id: web.id, seq: 5 });
    await waitFor(() => {
      expect(readsOf(QUEUE)).toBe(before + 1);
    });
  });

  it("tells the person that one of their PRs moved", async () => {
    const prOf = (state: ItemState) => itemOf(124, state, { author: "ada", title: "Fix the login redirect" });
    vi.stubGlobal("EventSource", FakeEventSource);
    renderApp("/orgs/acme/repos/web/queues/main", {
      ...answers,
      ...answersOfMine("acme", [
        [200, pageOf("prs", [prOf("pending")])],
        [200, pageOf("prs", [prOf("testing")])],
      ]),
    });
    await screen.findByText("1st in line");

    FakeEventSource.opened[0]?.emit("queue_changed", { queue_id: web.id, seq: 5 });

    expect(await screen.findByText("#124 Fix the login redirect: testing")).toBeInTheDocument();
  });
});
