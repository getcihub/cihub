import { useSuspenseQueries } from "@tanstack/react-query";
import { type ErrorComponentProps, getRouteApi } from "@tanstack/react-router";
import { Suspense } from "react";

import { ServerError } from "@/components/errors/ServerError";
import {
  DrawerFrame,
  DrawerNote,
  DrawerSection,
  DrawerSectionTitle,
  DrawerText,
} from "@/components/pr/DrawerFrame";
import { History } from "@/components/pr/History";
import { ItemStateBadge } from "@/components/pr/ItemStateBadge";
import { Journey } from "@/components/pr/Journey";
import { PrActions } from "@/components/pr/PrActions";
import { RunSection, RunSectionSkeleton } from "@/components/pr/RunSection";
import { prReads } from "@/domain/pr/queries";
import { isEnded } from "@/domain/pr/words";
import { useSentNotice } from "@/domain/queue/live";
import { plural } from "@/lib/text";
import { Alert } from "@/ui/alert";
import { Prose } from "@/ui/prose";

/** The page of a PR: the page of its queue, with the drawer of the PR open. */

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
          #{item.number} · {item.author}
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
          <RunSection org={org} runId={item.run_id} />
        </Suspense>
      )}
      <History events={events.events} />
    </DrawerFrame>
  );
}

/** A PR that the queue never had: the drawer says so over the page of the queue. */
export function PrNotFound() {
  const { org, repo, queue, number } = route.useParams();

  return <DrawerFrame path={{ org, repo, queue }} title={`#${String(number)} is not in this queue`} />;
}

/** The PR could not be read: the drawer says what the server answered. */
export function PrDrawerError({ error, reset }: ErrorComponentProps) {
  const { org, repo, queue, number } = route.useParams();

  return (
    <DrawerFrame path={{ org, repo, queue }} title={`#${String(number)}`}>
      <ServerError error={error} reset={reset} />
    </DrawerFrame>
  );
}
