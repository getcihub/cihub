import { createFileRoute, notFound, stripSearchParams } from "@tanstack/react-router";
import { z } from "zod";

import { NotFoundPage } from "@/components/errors/not-found-page";
import { VIEWS, branchConfigOptions } from "@/domain/config/queries";
import { failureOf, isProblem } from "@/domain/problem";
import { repoOptions } from "@/domain/setup/queries";
import { RepoConfigPageError } from "./error";
import { RepoConfigPageLoading } from "./loading";
import { RepoConfigPage } from "./page";

const search = z.object({
  view: z.enum(VIEWS).default("builder").catch("builder"),
  // The entry of the file that the builder shows: `defaults`, or the branch of an entry of `queues`.
  entry: z.string().optional().catch(undefined),
});

export const Route = createFileRoute("/orgs/$org/repos/$repo/config")({
  validateSearch: search,
  // The builder is the page itself: its address carries no view.
  search: { middlewares: [stripSearchParams({ view: "builder" })] },
  // A repository that the person does not read is not found; a configuration that CIHub has not read yet is a
  // state of the page (`error.tsx`).
  loader: async ({ context: { queryClient }, params: { org, repo } }) => {
    const [read] = await Promise.allSettled([
      queryClient.query(branchConfigOptions(org, repo)),
      queryClient.query(repoOptions(org, repo)),
    ]);
    if (read.status === "rejected" && isProblem(failureOf(read.reason), "repo_not_found")) {
      // eslint-disable-next-line @typescript-eslint/only-throw-error -- the router stops on a thrown notFound
      throw notFound();
    }
  },
  remountDeps: ({ params }) => params,
  component: RepoConfigPage,
  pendingComponent: RepoConfigPageLoading,
  errorComponent: RepoConfigPageError,
  notFoundComponent: NotFoundPage,
});
