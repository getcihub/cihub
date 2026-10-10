import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { RouterProvider, createMemoryHistory, createRouter } from "@tanstack/react-router";
import { render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { client } from "./client/client.gen";
import { routeTree } from "./routeTree.gen";
import { ada, queueOf } from "./test/fixtures";

type Answer = [number, unknown];
/** The answers of the API by path; a list answers its items in order, then its last one. */
type Answers = Record<string, Answer | Answer[]>;

const problem = (status: number, code: string) => ({
  type: "about:blank",
  title: code,
  status,
  detail: "",
  code,
});
const signedIn: Answers = {
  "/api/v1/me": [200, ada],
  "/api/v1/orgs/acme/queues": [200, { queues: [queueOf("web", "main")] }],
  "/api/v1/orgs/acme-labs/queues": [200, { queues: [] }],
};

/** The dashboard at `path`, with the API answering `answers` by path, and 404 else. */
function renderApp(path: string, answers: Answers) {
  const calls = new Map<string, number>();
  const answerOf = (pathname: string): Answer => {
    const answer = answers[pathname] ?? [404, problem(404, "not_found")];
    const call = calls.get(pathname) ?? 0;
    calls.set(pathname, call + 1);
    if (typeof answer[0] === "number") {
      return answer as Answer;
    }
    const list = answer as Answer[];
    return list[Math.min(call, list.length - 1)] ?? [500, null];
  };

  // jsdom has no address for a relative URL of `fetch`.
  client.setConfig({ baseUrl: "http://localhost/api/v1" });
  vi.stubGlobal(
    "fetch",
    vi.fn((input: Request | string) => {
      const { pathname } = new URL(typeof input === "string" ? input : input.url);
      const [status, body] = answerOf(pathname);
      const headers = { "content-type": "application/json" };
      return Promise.resolve(new Response(JSON.stringify(body), { status, headers }));
    }),
  );
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const router = createRouter({
    routeTree,
    context: { queryClient },
    history: createMemoryHistory({ initialEntries: [path] }),
  });

  render(
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
    </QueryClientProvider>,
  );
  return router;
}

afterEach(() => {
  vi.unstubAllGlobals();
  localStorage.clear();
});

describe("the routes of the dashboard", () => {
  it("send a person without a session to the sign-in", async () => {
    const router = renderApp("/", { "/api/v1/me": [401, problem(401, "unauthorized")] });

    expect(await screen.findByRole("heading", { name: "Sign in" })).toBeInTheDocument();
    expect(router.state.location.pathname).toBe("/sign-in");
  });

  it("open the organization that the person opened last, whatever its case", async () => {
    localStorage.setItem("org", "ACME-LABS");
    const router = renderApp("/", signedIn);

    await waitFor(() => {
      expect(router.state.location.pathname).toBe("/orgs/acme-labs");
    });
    expect(await screen.findByRole("heading", { name: "No queue yet" })).toBeInTheDocument();
  });

  it("say that an organization without a role is out of reach", async () => {
    renderApp("/orgs/globex", signedIn);

    expect(
      await screen.findByRole("heading", { level: 1, name: "You have no access to globex" }),
    ).toBeInTheDocument();
  });

  it("say that a repository that has no queue is not found", async () => {
    renderApp("/orgs/acme/repos/nope", signedIn);

    expect(await screen.findByRole("heading", { level: 1, name: "Page not found" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Repository: All repositories" })).toBeInTheDocument();
  });

  it("send a person whose session expired to the sign-in, then back to the page", async () => {
    const router = renderApp("/orgs/acme", {
      "/api/v1/me": [
        [200, ada],
        [401, problem(401, "unauthorized")],
      ],
      "/api/v1/orgs/acme/queues": [401, problem(401, "unauthorized")],
    });

    await waitFor(() => {
      expect(router.state.location.pathname).toBe("/sign-in");
    });
    expect(router.state.location.search).toEqual({ next: "/orgs/acme" });
  });
});
