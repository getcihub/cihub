import { useSuspenseQuery } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";

import { listOrgQueuesOptions } from "../client/@tanstack/react-query.gen";
import { READ_AGAIN_EVERY } from "../live";
import { QueueCards } from "../overview/QueueCards";
import { NotFoundPage } from "../pages/NotFoundPage";
import { ServerErrorPage } from "../pages/ServerErrorPage";
import styles from "../ui/ui.module.css";

export const Route = createFileRoute("/orgs/$org/repos/$repo/")({
  component: RepoPage,
  // A failed read shows under the frame, which stays.
  errorComponent: ServerErrorPage,
});

function RepoPage() {
  const { org, repo } = Route.useParams();
  const { data } = useSuspenseQuery({
    ...listOrgQueuesOptions({ path: { org } }),
    refetchInterval: READ_AGAIN_EVERY,
  });
  const queues = data.queues.filter((candidate) => candidate.repo === repo);

  if (queues.length === 0) {
    return <NotFoundPage />;
  }

  return (
    <main className={styles.page}>
      <div className={styles.head}>
        <div>
          <div className={styles.sub}>
            {org}/{repo}
          </div>
          <h1>Queues</h1>
        </div>
      </div>
      <QueueCards org={org} queues={queues} />
    </main>
  );
}
