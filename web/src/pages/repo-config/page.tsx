import { useSuspenseQuery } from "@tanstack/react-query";
import { Link, getRouteApi } from "@tanstack/react-router";
import { useState } from "react";

import type { BranchConfig, ConfigDocument } from "@/client/types.gen";
import { WarningIcon } from "@/components/icons/warning-icon";
import { Alert, AlertContent, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Page } from "@/components/ui/page";
import { Prose } from "@/components/ui/prose";
import { newQueue } from "@/domain/config/document";
import { DEFAULTS_ENTRY, type View, branchConfigOptions } from "@/domain/config/queries";
import { useReadAgain } from "@/hooks/use-read-again";
import { useRepoPermission } from "@/hooks/use-repo-permission";
import { cn } from "@/lib/cn";
import { AddQueueModal } from "./components/add-queue-modal";
import { EntryBuilder } from "./components/entry-builder";
import { EntryNav } from "./components/entry-nav";
import { PendingBar } from "./components/pending-bar";
import { ProposalBanner } from "./components/proposal-banner";
import { ProposeModal } from "./components/propose-modal";
import { RepoConfigHead } from "./components/repo-config-head";
import { YamlView } from "./components/yaml-view";
import { useConfigDraft } from "./use-config-draft";

const route = getRouteApi("/orgs/$org/repos/$repo/config");

type Dialog = "add" | "propose" | null;

/**
 * The configuration file of the default branch of a repository: the builder of each entry, each setting with its
 * value and its source, or the YAML. An admin of the repository changes the file in a draft, which the server
 * writes and checks, and proposes it as a pull request; the page reads again while CIHub opens it.
 */
export function RepoConfigPage() {
  const { org, repo } = route.useParams();
  const { view, entry } = route.useSearch();
  const navigate = route.useNavigate();
  const canEdit = useRepoPermission(org, repo).admin;
  const wait = useReadAgain((data: BranchConfig | undefined) => data?.proposal?.state === "requested");
  const { data: config } = useSuspenseQuery({
    ...branchConfigOptions(org, repo),
    refetchInterval: wait.refetchInterval,
  });
  const draft = useConfigDraft(org, repo, config, canEdit);
  const [dialog, setDialog] = useState<Dialog>(null);
  const chosen = chosenEntry(config, draft.document, entry);
  const goTo = (next: string) => {
    void navigate({ search: (search) => ({ ...search, entry: next }), resetScroll: false });
  };

  return (
    // The bar of the changes floats over the bottom of the page: the page keeps room for it.
    <Page className={cn(draft.isChanged && "pb-20")}>
      <RepoConfigHead org={org} repo={repo} />
      {config.state === "invalid" && (
        <Alert variant="destructive">
          <WarningIcon />
          <AlertContent>
            <AlertTitle>
              The file is not valid{config.line ? `, line ${String(config.line)}` : ""}:
            </AlertTitle>
            <AlertDescription>
              <Prose text={config.error ?? ""} />.{" "}
              {canEdit
                ? "The builder starts from the defaults; a proposed change writes the file again and fixes it."
                : `Each queue keeps its last valid file until a fix lands on ${config.branch}.`}
            </AlertDescription>
          </AlertContent>
        </Alert>
      )}
      {config.proposal && <ProposalBanner proposal={config.proposal} branch={config.branch} />}
      {!canEdit && (
        <Alert variant="warning" isBanner role="status">
          <WarningIcon />
          <AlertContent>
            <AlertTitle>Read only.</AlertTitle>
            <AlertDescription>
              Only an admin of {repo} proposes a change of the configuration.
            </AlertDescription>
          </AlertContent>
        </Alert>
      )}
      {draft.preview?.state === "invalid" && (
        <Alert variant="destructive">
          <WarningIcon />
          <AlertContent>
            <AlertTitle>
              The draft is not valid{draft.preview.line ? `, line ${String(draft.preview.line)}` : ""}:
            </AlertTitle>
            <AlertDescription>
              <Prose text={draft.preview.error ?? ""} />.
            </AlertDescription>
          </AlertContent>
        </Alert>
      )}
      <div className="flex flex-wrap items-center gap-3">
        <ViewTabs org={org} repo={repo} view={view} />
        <span className="ml-auto text-md text-muted-foreground">
          <FileSource config={config} />
        </span>
      </div>
      {view === "yaml" ? (
        <YamlView config={config} draft={draft} />
      ) : config.state === "invalid" && !canEdit ? (
        <p className="text-md text-secondary-foreground">
          The builder shows a valid file only. Fix the file on <code>{config.branch}</code>; its{" "}
          <Link
            to="/orgs/$org/repos/$repo/config"
            params={{ org, repo }}
            search={{ view: "yaml" }}
            className="text-primary"
          >
            YAML
          </Link>{" "}
          shows the line of the error.
        </p>
      ) : (
        <div className="grid gap-6 lg:grid-cols-[13rem_minmax(0,1fr)]">
          <EntryNav
            org={org}
            repo={repo}
            branch={config.branch}
            document={draft.document}
            entry={chosen}
            onAdd={
              canEdit
                ? () => {
                    setDialog("add");
                  }
                : null
            }
          />
          <EntryBuilder
            key={chosen}
            org={org}
            repo={repo}
            config={config}
            draft={draft}
            entry={chosen}
            canEdit={canEdit}
            onEntryRemoved={() => {
              goTo(DEFAULTS_ENTRY);
            }}
          />
        </div>
      )}
      {draft.isChanged && (
        <PendingBar
          org={org}
          repo={repo}
          view={view}
          draft={draft}
          onPropose={
            canEdit && config.sha
              ? () => {
                  setDialog("propose");
                }
              : null
          }
        />
      )}
      {dialog === "add" && (
        <AddQueueModal
          branch={config.branch}
          entries={draft.document.queues.map((queue) => queue.branch)}
          onClose={() => {
            setDialog(null);
          }}
          onAdd={(added) => {
            draft.change((copy) => {
              copy.queues.push(newQueue(added));
            });
            setDialog(null);
            goTo(added);
          }}
        />
      )}
      {dialog === "propose" && (
        <ProposeModal
          org={org}
          repo={repo}
          config={config}
          draft={draft}
          onClose={() => {
            setDialog(null);
          }}
          onProposed={() => {
            setDialog(null);
            draft.undo();
            wait.restart();
          }}
        />
      )}
    </Page>
  );
}

/**
 * The entry of the address when the document has it; else the queue of the branch of the file, the first queue,
 * or `defaults`.
 */
function chosenEntry(config: BranchConfig, document: ConfigDocument, entry: string | undefined): string {
  if (entry === DEFAULTS_ENTRY || document.queues.some((queue) => queue.branch === entry)) {
    return entry ?? DEFAULTS_ENTRY;
  }

  const own = document.queues.find((queue) => queue.branch === config.branch) ?? document.queues[0];
  return own?.branch ?? DEFAULTS_ENTRY;
}

/** Which file the page shows: its path, its commit and its branch. */
function FileSource({ config }: { config: BranchConfig }) {
  const sha = config.sha?.slice(0, 7);
  if (config.state === "none") {
    return (
      <>
        No <code>.cihub.yaml</code> on <code>{config.branch}</code>: the values of CIHub
      </>
    );
  }

  return (
    <>
      <code>
        {config.path}
        {sha && ` @ ${sha}`}
      </code>{" "}
      on <code>{config.branch}</code>
    </>
  );
}

const VIEW_LABELS: Record<View, string> = { builder: "Builder", yaml: "YAML" };

/** The two views of the file: the builder, or the YAML. */
function ViewTabs({ org, repo, view }: { org: string; repo: string; view: View }) {
  return (
    <nav className="flex gap-1" aria-label="View">
      {(["builder", "yaml"] as const).map((candidate) => (
        <Link
          key={candidate}
          to="/orgs/$org/repos/$repo/config"
          params={{ org, repo }}
          search={(search) => ({ ...search, view: candidate })}
          resetScroll={false}
          aria-current={candidate === view ? "page" : undefined}
          className={cn(
            "rounded-sm px-2.5 py-1 text-base font-medium text-secondary-foreground hover:bg-muted",
            candidate === view && "bg-primary-muted text-primary hover:bg-primary-muted",
          )}
        >
          {VIEW_LABELS[candidate]}
        </Link>
      ))}
    </nav>
  );
}
