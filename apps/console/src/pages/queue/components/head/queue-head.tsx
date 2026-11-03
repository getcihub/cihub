import type { ReactNode } from "react";

import type { Queue } from "@/client/types.gen";
import { QueueStateBadge } from "@/components/queue/queue-state-badge";
import { PageBand } from "@cihub/ui/components/page";
import { Toast } from "@cihub/ui/components/toast";
import type { QueuePath } from "@/domain/queue/queries";
import { useRepoPermission } from "@/hooks/use-repo-permission";
import type { SentNotice } from "@/hooks/use-sent-notice";
import { QueueActions } from "./queue-actions";
import { SubmitPr } from "./submit-pr";

/**
 * The head of the page of a queue, in the band under the top bar: its tabs, then its state and its actions; and, in
 * a toast, what an action of the page sent, until the queue changes: the page does not move under it. The page
 * shows it even when its other reads fail.
 */
export function QueueHead({
  path,
  queue,
  sent: { notice, showNotice },
  children,
}: {
  path: QueuePath;
  queue: Queue;
  sent: SentNotice;
  /** The tabs of the page. */
  children: ReactNode;
}) {
  const permission = useRepoPermission(path.org, path.repo);

  return (
    <>
      <PageBand>
        {/* The top bar already shows where the page is: the title stays for a screen reader. */}
        <h1 className="sr-only">{queue.target_branch}</h1>
        {children}
        {/* On a narrow screen, the state and the actions come first, and the tabs scroll under them. */}
        <div className="ml-auto flex items-center gap-2 pb-1.5 max-lg:order-first max-lg:ml-0 max-lg:flex-wrap max-lg:pb-0">
          <QueueStateBadge queue={queue} />
          <QueueActions path={path} queue={queue} isAdmin={permission.admin} />
          <SubmitPr path={path} isDisabled={queue.is_suspended || !permission.write} onSent={showNotice} />
        </div>
      </PageBand>
      <Toast>{notice}</Toast>
    </>
  );
}
