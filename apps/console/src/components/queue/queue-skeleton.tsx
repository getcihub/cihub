import { Card, CardHeader } from "@cihub/ui/components/card";
import { Page, PageBand } from "@cihub/ui/components/page";
import { LoadingStatus, Skeleton } from "@cihub/ui/components/skeleton";

/** The rows of the items while they load: the columns of `ItemTabs`. */
export function ItemRowsSkeleton({ count = 5 }: { count?: number }) {
  return (
    <div className="divide-y divide-divider">
      {Array.from({ length: count }, (_, row) => (
        <div key={row} className="flex items-center gap-8 px-4 py-3">
          <Skeleton className="w-3.5 flex-none" />
          <div className="flex w-80 flex-none items-center gap-2.5">
            <Skeleton className="size-5.5 flex-none rounded-full" />
            <div className="grid gap-1.5">
              <Skeleton className="w-47.5" />
              <Skeleton className="h-2.25 w-22.5" />
            </div>
          </div>
          <Skeleton className="h-5 w-20 rounded-full" />
          <Skeleton className="w-60 max-md:hidden" />
          <Skeleton className="w-12.5" />
          <Skeleton className="w-10" />
        </div>
      ))}
    </div>
  );
}

/** The page of a queue while it loads: its tabs and actions, its counters, its settings, its graph and its items. */
export function QueueSkeleton() {
  return (
    <>
      <PageBand className="h-9.75 pb-1.5 max-lg:h-auto max-lg:pb-0">
        <div className="flex gap-5 max-lg:h-9.25 max-lg:items-center">
          <Skeleton className="w-16" />
          <Skeleton className="w-16" />
        </div>
        <div className="flex gap-2 max-lg:order-first">
          <Skeleton className="h-8 w-20" />
          <Skeleton className="h-8 w-20" />
          <Skeleton className="h-8 w-40" />
        </div>
      </PageBand>
      <Page>
        <LoadingStatus />
        <div className="grid grid-cols-4 gap-3 max-md:grid-cols-2">
          {[1, 2, 3, 4].map((stat) => (
            <Card key={stat} className="grid gap-2.5 px-3.5 py-3">
              <Skeleton className="h-2.25 w-20" />
              <Skeleton className="h-7 w-12.5" />
              <Skeleton className="h-2.25 w-30" />
            </Card>
          ))}
        </div>
        <Card className="h-15.5 p-4.5">
          <Skeleton className="w-[60%]" />
        </Card>
        <Card>
          <CardHeader>
            <Skeleton className="w-22.5" />
          </CardHeader>
          <div className="h-40 p-4.5">
            <Skeleton className="h-30" />
          </div>
        </Card>
        <Card>
          <ItemRowsSkeleton />
        </Card>
      </Page>
    </>
  );
}
