import { Card } from "@cihub/ui/components/card";
import { LoadingStatus, Skeleton } from "@cihub/ui/components/skeleton";

/** The cards of the queues while they load: four, in the grid of `QueueCards`. */
export function QueueCardsSkeleton() {
  return (
    <div className="grid grid-cols-[repeat(auto-fill,minmax(300px,1fr))] gap-3">
      <LoadingStatus />
      {[1, 2, 3, 4].map((card) => (
        <Card key={card} className="grid content-start gap-3 px-4 py-3.5">
          <Skeleton className="w-30" />
          <Skeleton className="h-6.5 w-45" />
          <Skeleton className="w-[70%]" />
        </Card>
      ))}
    </div>
  );
}
