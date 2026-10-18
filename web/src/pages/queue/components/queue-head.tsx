import type { Queue } from "@/client/types.gen";
import { QueueStateBadge } from "@/components/queue/queue-state-badge";
import { PageHeader, PageLocation, PageTitle } from "@/components/ui/page";
import { Toast } from "@/components/ui/toast";
import type { QueuePath } from "@/domain/queue/queries";
import { useRepoPermission } from "@/hooks/use-repo-permission";
import type { SentNotice } from "@/hooks/use-sent-notice";
import { QueueActions } from "./queue-actions";
import { SubmitPr } from "./submit-pr";

/**
 * The head of the page of a queue: where it is, its name and state, and its actions; and, in a toast, what an action
 * of the page sent, until the queue changes: the page does not move under it. The page shows it even when its other
 * reads fail.
 */
export function QueueHead({
  path,
  queue,
  sent: { notice, showNotice },
}: {
  path: QueuePath;
  queue: Queue;
  sent: SentNotice;
}) {
  const permission = useRepoPermission(path.org, path.repo);

  return (
    <>
      <PageHeader>
        <div>
          <PageLocation>
            {path.org}/{path.repo}
          </PageLocation>
          <div className="flex items-center gap-3 tracking-[-0.3px]">
            <PageTitle>{queue.target_branch}</PageTitle>
            <QueueStateBadge queue={queue} />
          </div>
        </div>
        <div className="ml-auto flex gap-2">
          <QueueActions path={path} queue={queue} isAdmin={permission.admin} />
          <SubmitPr path={path} isDisabled={queue.is_suspended || !permission.write} onSent={showNotice} />
        </div>
      </PageHeader>
      <Toast>{notice}</Toast>
    </>
  );
}
