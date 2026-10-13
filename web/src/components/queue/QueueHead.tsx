import type { Queue } from "@/client/types.gen";
import { useSentNotice } from "@/domain/queue/live";
import type { QueuePath } from "@/domain/queue/queries";
import { membershipOf, useMe } from "@/domain/session/session";
import { Alert } from "@/ui/alert";
import { PageHeader, PageLocation, PageTitle } from "@/ui/page";
import { QueueActions } from "./QueueActions";
import { QueueStateBadge } from "./QueueStateBadge";
import { SubmitPr } from "./SubmitPr";

/**
 * The head of the page of a queue: where it is, its name and state, and its actions; then what an action sent,
 * until the queue changes. The page shows it even when its other reads fail.
 */
export function QueueHead({ path, queue }: { path: QueuePath; queue: Queue }) {
  const me = useMe();
  const isAdmin = membershipOf(me, path.org)?.role === "admin";
  const { notice, showNotice } = useSentNotice(queue.id);

  return (
    <>
      <PageHeader>
        <div>
          <PageLocation>
            {path.org}/{path.repo}
          </PageLocation>
          <div className="flex items-center gap-3">
            <PageTitle>{queue.name}</PageTitle>
            <QueueStateBadge queue={queue} />
          </div>
        </div>
        <div className="ml-auto flex gap-2">
          <QueueActions path={path} queue={queue} isAdmin={isAdmin} onSent={showNotice} />
          <SubmitPr path={path} isDisabled={queue.access === "suspended"} onSent={showNotice} />
        </div>
      </PageHeader>
      {notice && <Alert role="status">{notice}</Alert>}
    </>
  );
}
