import { Link } from "@tanstack/react-router";

import { cn } from "@/lib/cn";

const TAB =
  "-mb-px inline-flex items-center border-b-2 border-transparent px-0.5 pt-1.5 pb-2.25 font-medium whitespace-nowrap text-secondary-foreground hover:text-foreground";
const CURRENT = "border-primary text-foreground";

/** The pages of a repository: its queues and setup, then the configuration file of its default branch. */
export function RepoTabs({
  org,
  repo,
  current,
}: {
  org: string;
  repo: string;
  current: "overview" | "config";
}) {
  return (
    <nav className="flex gap-5 border-b border-border" aria-label="Repository">
      <Link
        className={cn(TAB, current === "overview" && CURRENT)}
        to="/orgs/$org/repos/$repo"
        params={{ org, repo }}
        aria-current={current === "overview" ? "page" : undefined}
      >
        Overview
      </Link>
      <Link
        className={cn(TAB, current === "config" && CURRENT)}
        to="/orgs/$org/repos/$repo/config"
        params={{ org, repo }}
        aria-current={current === "config" ? "page" : undefined}
      >
        Configuration
      </Link>
    </nav>
  );
}
