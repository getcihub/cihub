import { type ErrorComponentProps, getRouteApi } from "@tanstack/react-router";

import { ServerErrorPage } from "@/components/errors/server-error-page";
import { Card } from "@/components/ui/card";
import { EmptyState } from "@/components/ui/empty-state";
import { QueueIllustration } from "@/components/ui/illustrations";
import { Page } from "@/components/ui/page";
import { failureOf, isProblem } from "@/domain/problem";
import { RepoConfigHead } from "./components/repo-config-head";

const route = getRouteApi("/orgs/$org/repos/$repo/config");

/**
 * The configuration could not be read: CIHub has not read the file yet, or the server failed. The head of the
 * page stays.
 */
export function RepoConfigPageError(props: ErrorComponentProps) {
  const { org, repo } = route.useParams();
  const head = <RepoConfigHead org={org} repo={repo} />;

  if (!isProblem(failureOf(props.error), "config_not_found")) {
    return <ServerErrorPage {...props} head={head} />;
  }

  return (
    <Page>
      {head}
      <Card>
        <EmptyState illustration={<QueueIllustration />} title="Not read yet">
          CIHub reads the configuration file of the default branch with the setup of the repository. It shows
          here once that read ends.
        </EmptyState>
      </Card>
    </Page>
  );
}
