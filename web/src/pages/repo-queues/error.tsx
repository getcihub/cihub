import { type ErrorComponentProps, getRouteApi } from "@tanstack/react-router";

import { ServerErrorPage } from "@/components/errors/server-error-page";
import { OverviewHead } from "@/components/queue/overview-head";

const route = getRouteApi("/orgs/$org/repos/$repo/");

/** The queues could not be read: the head of the page stays, with what the server answered. */
export function RepoQueuesPageError(props: ErrorComponentProps) {
  const { org, repo } = route.useParams();

  return <ServerErrorPage {...props} head={<OverviewHead where={`${org}/${repo}`} />} />;
}
