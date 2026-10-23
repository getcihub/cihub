import { Link, useNavigate } from "@tanstack/react-router";
import type { ReactNode } from "react";

import type { ConfigDocument } from "@/client/types.gen";
import { PlusIcon } from "@/components/icons/plus-icon";
import { OptionPicker } from "@/components/ui/option-picker";
import { isGlob } from "@/domain/config/document";
import { DEFAULTS_ENTRY } from "@/domain/config/queries";
import { cn } from "@/lib/cn";

/** The sections of the builder, and those that only a queue has. */
const SECTIONS: { id: string; title: string; isQueueOnly?: boolean }[] = [
  { id: "cfg-strategy", title: "Strategy" },
  { id: "cfg-testing", title: "Testing" },
  { id: "cfg-batches", title: "Batches" },
  { id: "cfg-failures", title: "Failures" },
  { id: "cfg-conditions", title: "Conditions" },
  { id: "cfg-checks", title: "Required checks" },
  { id: "cfg-waiting", title: "Waiting" },
  { id: "cfg-priority", title: "Priority", isQueueOnly: true },
  { id: "cfg-profiles", title: "Profiles", isQueueOnly: true },
  { id: "cfg-merging", title: "Merging" },
  { id: "cfg-comments", title: "On the pull requests" },
  { id: "cfg-test-branches", title: "Test branches" },
];

function sectionsOf(entry: string) {
  return SECTIONS.filter((section) => entry !== DEFAULTS_ENTRY || !section.isQueueOnly);
}

function jumpTo(id: string) {
  const reduced = matchMedia("(prefers-reduced-motion: reduce)").matches;
  document.getElementById(id)?.scrollIntoView({ behavior: reduced ? "auto" : "smooth", block: "start" });
}

/** What an entry covers, under its name. */
function hintOf(entry: string, branch: string): string {
  if (entry === branch) {
    return "the branch of this file";
  }
  return isGlob(entry) ? "each branch that matches" : "another branch";
}

/**
 * The entries of the file: `defaults`, then each queue, the sections of the chosen one under it, and "Add a queue".
 * On a narrow screen, a bar with two pickers, the entry and the section, takes its place.
 */
export function EntryNav({
  org,
  repo,
  branch,
  document: file,
  entry,
  onAdd,
}: {
  org: string;
  repo: string;
  branch: string;
  document: ConfigDocument;
  entry: string;
  /** Adds a queue; none for a person who cannot propose. */
  onAdd: (() => void) | null;
}) {
  const navigate = useNavigate();
  const sections = (
    <div className="my-0.5 mb-1.5 ml-2.5 grid border-l border-divider pl-2">
      {sectionsOf(entry).map((section) => (
        <button
          key={section.id}
          type="button"
          className="cursor-pointer rounded-sm px-2 py-1 text-left text-md text-muted-foreground hover:bg-muted hover:text-foreground"
          onClick={() => {
            jumpTo(section.id);
          }}
        >
          {section.title}
        </button>
      ))}
    </div>
  );

  return (
    <>
      <nav
        aria-label="The file"
        className="sticky top-17 grid max-h-[calc(100vh-84px)] content-start gap-0.5 overflow-y-auto rounded-md border border-border bg-card p-2 max-lg:hidden"
      >
        <EntryLink
          org={org}
          repo={repo}
          entry={DEFAULTS_ENTRY}
          isCurrent={entry === DEFAULTS_ENTRY}
          hint="every queue of the file"
        >
          Defaults
        </EntryLink>
        {entry === DEFAULTS_ENTRY && sections}
        <div className="mx-2 mt-2.5 mb-1 text-2xs tracking-wider text-muted-foreground uppercase">
          Queues <span className="ml-1">{file.queues.length}</span>
        </div>
        {file.queues.length === 0 && (
          <p className="px-2 text-md text-muted-foreground">None: each branch takes the defaults.</p>
        )}
        {file.queues.map((queue) => (
          <div key={queue.branch} className="grid">
            <EntryLink
              org={org}
              repo={repo}
              entry={queue.branch}
              isCurrent={entry === queue.branch}
              hint={hintOf(queue.branch, branch)}
            >
              <code>{queue.branch}</code>
            </EntryLink>
            {entry === queue.branch && sections}
          </div>
        ))}
        {onAdd && (
          <button
            type="button"
            className="mt-1 flex cursor-pointer items-center gap-1.5 rounded-sm border border-dashed border-border px-2 py-1.75 text-base text-primary hover:border-primary-border hover:bg-primary-muted [&>svg]:size-3"
            onClick={onAdd}
          >
            <PlusIcon />
            Add a queue
          </button>
        )}
      </nav>
      <div
        className="sticky top-14 z-20 flex gap-2 bg-background py-2 lg:hidden"
        role="group"
        aria-label="The file"
      >
        <div className="min-w-0 flex-1">
          <OptionPicker
            label="Item of the file"
            isWide
            value={entry}
            options={[
              { value: DEFAULTS_ENTRY, label: "Defaults", hint: "every queue of the file" },
              ...file.queues.map((queue) => ({
                value: queue.branch,
                label: <code>{queue.branch}</code>,
                hint: hintOf(queue.branch, branch),
              })),
            ]}
            onChange={(next) => {
              void navigate({
                to: "/orgs/$org/repos/$repo/config",
                params: { org, repo },
                search: (search) => ({ ...search, entry: next }),
                resetScroll: false,
              });
            }}
          />
        </div>
        <div className="min-w-0 flex-1">
          <OptionPicker
            label="Section"
            isWide
            align="end"
            value="cfg-strategy"
            options={sectionsOf(entry).map((section) => ({ value: section.id, label: section.title }))}
            onChange={jumpTo}
          />
        </div>
        {onAdd && (
          <button
            type="button"
            className="flex size-8 flex-none cursor-pointer items-center justify-center rounded-sm border border-border bg-card [&>svg]:size-3.5"
            aria-label="Add a queue"
            onClick={onAdd}
          >
            <PlusIcon />
          </button>
        )}
      </div>
    </>
  );
}

function EntryLink({
  org,
  repo,
  entry,
  isCurrent,
  hint,
  children,
}: {
  org: string;
  repo: string;
  entry: string;
  isCurrent: boolean;
  hint: string;
  children: ReactNode;
}) {
  return (
    <Link
      to="/orgs/$org/repos/$repo/config"
      params={{ org, repo }}
      search={(search) => ({ ...search, entry })}
      resetScroll={false}
      aria-current={isCurrent ? "page" : undefined}
      className={cn(
        "grid rounded-sm px-2 py-1.75 text-left font-medium text-foreground hover:bg-muted",
        isCurrent && "bg-primary-muted shadow-current hover:bg-primary-muted",
      )}
    >
      <span className="justify-self-start">{children}</span>
      <span className="truncate text-sm font-normal text-muted-foreground">{hint}</span>
    </Link>
  );
}
