import { useSuspenseQuery } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";

import { listOrgQueuesOptions } from "../client/@tanstack/react-query.gen";
import { NotFoundPage } from "../pages/NotFoundPage";
import styles from "../ui/ui.module.css";

export const Route = createFileRoute("/orgs/$org/repos/$repo/")({
  component: RepoPage,
});

// The cards of the queues of the repository come with lot 10b.
function RepoPage() {
  const { org, repo } = Route.useParams();
  const { data } = useSuspenseQuery(listOrgQueuesOptions({ path: { org } }));

  if (!data.queues.some((candidate) => candidate.repo === repo)) {
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
    </main>
  );
}
