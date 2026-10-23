import { OverviewHead } from "@/components/queue/overview-head";
import { RepoTabs } from "@/components/repo/repo-tabs";

/** The head of the configuration of a repository: its name, then the pages of the repository. */
export function RepoConfigHead({ org, repo }: { org: string; repo: string }) {
  return (
    <>
      <OverviewHead where={org} title={repo} />
      <RepoTabs org={org} repo={repo} current="config" />
    </>
  );
}
