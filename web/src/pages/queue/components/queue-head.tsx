import type { Queue } from "@/client/types.gen";
import { QueueStateBadge } from "@/components/queue/queue-state-badge";
import { Alert } from "@/components/ui/alert";
import { PageHeader, PageLocation, PageTitle } from "@/components/ui/page";
import type { QueuePath } from "@/domain/queue/queries";
import { membershipOf } from "@/domain/session/session";
import { useMe } from "@/hooks/use-me";
import { useSentNotice } from "@/hooks/use-sent-notice";
import { QueueActions } from "./queue-actions";
import { SubmitPr } from "./submit-pr";

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
