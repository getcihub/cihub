import { useSuspenseQueries } from "@tanstack/react-query";
import { getRouteApi } from "@tanstack/react-router";
import { Suspense } from "react";

import { ItemStateBadge } from "@/components/pr/item-state-badge";
import { Alert } from "@/components/ui/alert";
import { Prose } from "@/components/ui/prose";
import { prReads } from "@/domain/pr/queries";
import { isEnded } from "@/domain/pr/words";
import { useSentNotice } from "@/hooks/use-sent-notice";
import { plural } from "@/lib/text";
import {
  DrawerFrame,
  DrawerNote,
  DrawerSection,
  DrawerSectionTitle,
  DrawerText,
} from "@/pages/pr/components/drawer-frame";
import { History } from "@/pages/pr/components/history";
import { Journey } from "@/pages/pr/components/journey";
import { PrActions } from "@/pages/pr/components/pr-actions";
import { RunSection, RunSectionSkeleton } from "@/pages/pr/components/run-section";

const route = getRouteApi("/orgs/$org/repos/$repo/queues/$queue/prs/$number");

/** The drawer of a PR: where it is, why it waits or how it ended, its run and its history. */
export function PrDrawer() {
  const { org, repo, queue, number } = route.useParams();
  const path = { org, repo, queue };
  const [{ data: item }, { data: events }] = useSuspenseQueries({ queries: prReads(path, number) });
  const ahead = item.ahead.length;
  const lastWait = events.events.findLast((event) => event.type === "item.readiness_changed");
  const { notice, showNotice } = useSentNotice(item.queue_id);
  const hasEnded = isEnded(item.state);

  return (
    <DrawerFrame
      path={path}
      title={item.title ?? `#${String(item.number)}`}
      meta={
        <>
          #{item.number} · {item.author.login}
          {!hasEnded && item.position != null && ` · position ${String(ahead + 1)}`}
        </>
      }
      head={
        <>
          <div className="flex flex-wrap items-center justify-between gap-2">
            <ItemStateBadge state={item.state} />
            {!hasEnded && <PrActions path={path} item={item} onSent={showNotice} />}
          </div>
          {notice && (
            <Alert role="status" className="mt-2.5 px-2.5 py-2">
              {notice}
            </Alert>
          )}
        </>
      }
    >
      <DrawerSection aria-label="Where it is">
        <Journey item={item} />
        {!hasEnded && item.state !== "not_ready" && (
          <DrawerNote>
            {ahead === 0
              ? "Nothing merges before it"
              : `${plural(ahead, "pull request", "pull requests")} merge before it`}
          </DrawerNote>
        )}
      </DrawerSection>
      {item.state === "not_ready" && (
        <DrawerSection aria-labelledby="wait-title">
          <DrawerSectionTitle id="wait-title">Why it waits</DrawerSectionTitle>
          <DrawerText>
            <Prose text={lastWait?.description ?? item.state_description} />
          </DrawerText>
          <DrawerNote>It enters the queue by itself once nothing blocks it.</DrawerNote>
        </DrawerSection>
      )}
      {hasEnded && (
        <DrawerSection aria-labelledby="outcome-title">
          <DrawerSectionTitle id="outcome-title">Outcome</DrawerSectionTitle>
          <DrawerText>{item.state_description}</DrawerText>
        </DrawerSection>
      )}
      {item.run_id && (
        <Suspense fallback={<RunSectionSkeleton />}>
          <RunSection org={org} repo={repo} runId={item.run_id} />
        </Suspense>
      )}
      <History events={events.events} />
    </DrawerFrame>
  );
}
