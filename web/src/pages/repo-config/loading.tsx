import { getRouteApi } from "@tanstack/react-router";

import { Card } from "@/components/ui/card";
import { Page } from "@/components/ui/page";
import { LoadingStatus, Skeleton } from "@/components/ui/skeleton";
import { RepoConfigHead } from "./components/repo-config-head";

const route = getRouteApi("/orgs/$org/repos/$repo/config");

/** The shapes of the builder: the entries of the file, then the rows of the settings. */
export function RepoConfigPageLoading() {
  const { org, repo } = route.useParams();

  return (
    <Page>
      <RepoConfigHead org={org} repo={repo} />
      <LoadingStatus />
      <div className="grid gap-5 lg:grid-cols-[13rem_minmax(0,1fr)]">
        <div className="grid content-start gap-3 max-lg:hidden">
          <Skeleton className="w-24" />
          <Skeleton className="w-32" />
          <Skeleton className="w-20" />
        </div>
        <div className="grid gap-2">
          {[0, 1, 2, 3].map((row) => (
            <Card key={row} className="grid gap-2 p-4">
              <Skeleton className="w-40" />
              <Skeleton className="w-2/3" />
            </Card>
          ))}
        </div>
      </div>
    </Page>
  );
}
