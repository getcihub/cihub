import { OverviewHead } from "@/components/queue/overview-head";
import { QueueCardsSkeleton } from "@/components/queue/queue-cards-skeleton";
import { Page } from "@cihub/ui";

/** A list of queues while it loads: the head of the page, then the shapes of the cards. */
export function QueuesSkeleton({ where, title }: { where: string; title: string }) {
  return (
    <Page>
      <OverviewHead where={where} title={title} />
      <QueueCardsSkeleton />
    </Page>
  );
}
