import { Button, buttonVariants } from "@cihub/ui/components/button";
import { Card } from "@cihub/ui/components/card";
import { EmptyState } from "@cihub/ui/components/empty-state";
import { QueueIllustration } from "@cihub/ui/components/illustrations";

/** An organization whose App reaches no repository yet: add some on GitHub, or read them again. */
export function NoRepository({
  org,
  installUrl,
  checkAgain,
}: {
  org: string;
  installUrl: string | null | undefined;
  checkAgain: { run: () => void; isPending: boolean };
}) {
  return (
    <Card>
      <EmptyState
        illustration={<QueueIllustration />}
        title="No repository yet"
        actions={
          <>
            {installUrl && (
              <a className={buttonVariants({ variant: "primary" })} href={installUrl}>
                Add repositories
              </a>
            )}
            <Button isPending={checkAgain.isPending} pendingText="Checking" onClick={checkAgain.run}>
              Check again
            </Button>
          </>
        }
      >
        The App reaches no repository of {org} yet: it reaches only the repositories that an owner chose for
        it.
      </EmptyState>
    </Card>
  );
}
