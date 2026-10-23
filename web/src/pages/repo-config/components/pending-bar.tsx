import { Link } from "@tanstack/react-router";

import { Button } from "@/components/ui/button";
import { countChanges } from "@/domain/config/document";
import type { View } from "@/domain/config/queries";
import type { ConfigDraft } from "@/pages/repo-config/use-config-draft";

/**
 * The changes of the draft, at the bottom of the page: how many lines they add and remove, the other view of the
 * file, "Undo all", and "Propose as a pull request" for a person who can propose.
 */
export function PendingBar({
  org,
  repo,
  view,
  draft,
  onPropose,
}: {
  org: string;
  repo: string;
  view: View;
  draft: ConfigDraft;
  onPropose: (() => void) | null;
}) {
  const { added, removed } = countChanges(draft.preview?.diff ?? []);
  const isValid = draft.preview?.state === "valid";
  const other: View = view === "yaml" ? "builder" : "yaml";

  return (
    <div
      role="region"
      aria-label="Pending changes"
      className="fixed bottom-5 left-1/2 z-30 flex -translate-x-1/2 items-center gap-4 rounded-lg bg-foreground py-2 pr-2 pl-4 text-base whitespace-nowrap text-background shadow-lg max-md:right-3 max-md:left-3 max-md:translate-x-0 max-md:flex-wrap max-md:whitespace-normal"
    >
      <span>
        <b className="font-medium text-success">+{added}</b>{" "}
        <b className="font-medium text-destructive">−{removed}</b> pending{" "}
        {added + removed === 1 ? "change" : "changes"} to{" "}
        <code className="border-transparent bg-background/10 text-background">.cihub.yaml</code>
      </span>
      <Link
        className="cursor-pointer text-primary-border hover:underline"
        to="/orgs/$org/repos/$repo/config"
        params={{ org, repo }}
        search={(search) => ({ ...search, view: other })}
        resetScroll={false}
      >
        {other === "yaml" ? "View YAML" : "View builder"}
      </Link>
      <button
        type="button"
        className="cursor-pointer text-primary-border hover:underline"
        onClick={draft.undo}
      >
        Undo all
      </button>
      {onPropose && (
        <Button
          className="border-background bg-background text-foreground hover:border-background"
          disabled={!isValid}
          onClick={onPropose}
        >
          Propose as a pull request
        </Button>
      )}
    </div>
  );
}
