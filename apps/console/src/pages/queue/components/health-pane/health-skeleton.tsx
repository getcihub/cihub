import { Card, CardHeader } from "@cihub/ui/components/card";
import { LoadingStatus, Skeleton } from "@cihub/ui/components/skeleton";

/** The Health while it loads: its verdict, its counters and its charts. */
export function HealthSkeleton() {
  return (
    <>
      <LoadingStatus />
      <Skeleton className="h-4 w-80" />
      <div className="grid grid-cols-3 gap-3 max-md:grid-cols-1">
        {[1, 2, 3].map((stat) => (
          <Card key={stat} className="grid gap-2.5 px-3.5 py-3">
            <Skeleton className="h-2.25 w-20" />
            <Skeleton className="h-6 w-15" />
            <Skeleton className="h-2.25 w-32.5" />
          </Card>
        ))}
      </div>
      <div className="grid grid-cols-2 gap-4 max-md:grid-cols-1">
        {[1, 2].map((chart) => (
          <Card key={chart}>
            <CardHeader>
              <Skeleton className="w-27.5" />
            </CardHeader>
            <div className="p-4">
              <Skeleton className="h-37.5" />
            </div>
          </Card>
        ))}
      </div>
    </>
  );
}
