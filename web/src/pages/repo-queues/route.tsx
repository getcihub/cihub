import { createFileRoute, notFound } from "@tanstack/react-router";

import { NotFoundPage } from "@/components/errors/not-found-page";
import { failureOf, isProblem } from "@/domain/problem";
import { orgQueuesOptions } from "@/domain/queue/queries";
import { repoOptions } from "@/domain/setup/queries";
import { RepoQueuesPageError } from "./error";
import { RepoQueuesPageLoading } from "./loading";
import { RepoQueuesPage } from "./page";

export const Route = createFileRoute("/orgs/$org/repos/$repo/")({
  // A repository that the person does not read is not found; the layout of the organization says why it cannot
  // read the others. The two reads leave together.
  loader: async ({ context: { queryClient }, params: { org, repo } }) => {
    const [read] = await Promise.allSettled([
      queryClient.query(repoOptions(org, repo)),
      queryClient.query(orgQueuesOptions(org)),
    ]);
    if (read.status === "rejected" && isProblem(failureOf(read.reason), "repo_not_found")) {
      // eslint-disable-next-line @typescript-eslint/only-throw-error -- the router stops on a thrown notFound
      throw notFound();
    }
  },
  // Another repository waits for its own reads.
  remountDeps: ({ params }) => params,
  component: RepoQueuesPage,
  pendingComponent: RepoQueuesPageLoading,
  errorComponent: RepoQueuesPageError,
  notFoundComponent: NotFoundPage,
});
