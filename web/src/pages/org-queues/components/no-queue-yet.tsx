import { ArrowIcon } from "@/components/icons/arrow-icon";
import { buttonVariants } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { EmptyState } from "@/components/ui/empty-state";

/** An organization without a queue: the first `/cihub merge` creates one, on a repository that the App reaches. */
export function NoQueueYet({ org, installUrl }: { org: string; installUrl?: string | null | undefined }) {
  return (
    <Card>
      <EmptyState
        illustration={null}
        title="No queue yet"
        actions={
          installUrl && (
            <a className={buttonVariants()} href={installUrl}>
              Choose the repositories of the App <ArrowIcon />
            </a>
          )
        }
      >
        The CIHub App is installed on <b>{org}</b>. The queue of a branch appears here at the first{" "}
        <code>/cihub merge</code> on one of its pull requests.
      </EmptyState>
    </Card>
  );
}
