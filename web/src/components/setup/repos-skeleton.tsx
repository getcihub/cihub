import { OverviewHead } from "@/components/queue/overview-head";
import { Card } from "@/components/ui/card";
import { Page } from "@/components/ui/page";
import { LoadingStatus, Skeleton } from "@/components/ui/skeleton";

/** The repositories of an organization while they load: the head of the page, then the shapes of six rows. */
export function ReposSkeleton({ org }: { org: string }) {
  return (
    <Page>
      <OverviewHead where={org} title="Repositories" />
      <Card className="p-0">
        <LoadingStatus />
        {[1, 2, 3, 4, 5, 6].map((row) => (
          <div
            key={row}
            className="flex h-11 items-center gap-4 border-b border-divider px-4 last:border-b-0"
          >
            <Skeleton className="w-45" />
            <Skeleton className="w-12" />
            <Skeleton className="ml-auto w-20" />
          </div>
        ))}
      </Card>
    </Page>
  );
}
