import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";

import { proposeConfigMutation } from "@/client/@tanstack/react-query.gen";
import type { BranchConfig } from "@/client/types.gen";
import { WarningIcon } from "@/components/icons/warning-icon";
import { Alert, AlertContent, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Modal, ModalDescription, ModalFooter } from "@/components/ui/modal";
import { completeDocument } from "@/domain/config/document";
import { branchConfigOptions } from "@/domain/config/queries";
import { failureOf, problemOf } from "@/domain/problem";
import type { ConfigDraft } from "@/pages/repo-config/use-config-draft";
import { DiffText } from "./yaml-view";

/** The message that the modal offers: what the change does to the file. */
function messageOf(config: BranchConfig): string {
  if (config.state === "invalid") {
    return "Fix the merge queue configuration";
  }
  return config.state === "none" ? "Configure the merge queue" : "Update the merge queue configuration";
}

/**
 * The proposal of the draft as a pull request: CIHub never changes the branch itself. The person writes the
 * message of the commit and reads the diff once more.
 */
export function ProposeModal({
  org,
  repo,
  config,
  draft,
  onClose,
  onProposed,
}: {
  org: string;
  repo: string;
  config: BranchConfig;
  draft: ConfigDraft;
  onClose: () => void;
  onProposed: () => void;
}) {
  const queryClient = useQueryClient();
  const [message, setMessage] = useState(messageOf(config));
  const propose = useMutation({
    ...proposeConfigMutation(),
    onSettled: () => queryClient.invalidateQueries({ queryKey: branchConfigOptions(org, repo).queryKey }),
    onSuccess: onProposed,
  });
  const problem = propose.error ? problemOf(failureOf(propose.error)) : null;
  const changes = (draft.preview?.diff ?? []).filter((line) => line.change !== "same");

  return (
    <Modal title="Propose the change as a pull request" onClose={onClose}>
      <ModalDescription>
        CIHub does not change <code>{config.branch}</code> directly: it pushes the branch{" "}
        <code>cihub-config/{config.branch}</code> and opens a pull request that you or a teammate review and
        merge.
      </ModalDescription>
      {config.state === "valid" && (
        <Alert variant="warning">
          <WarningIcon />
          <AlertContent>
            <AlertTitle>Comments in the file are lost.</AlertTitle>
            <AlertDescription>CIHub writes the whole file again; review the diff.</AlertDescription>
          </AlertContent>
        </Alert>
      )}
      <label className="grid gap-1 text-md font-medium">
        Commit message
        <textarea
          className="min-h-16 rounded-sm border border-border bg-card px-2 py-1.5 text-base font-normal"
          value={message}
          onChange={(event) => {
            setMessage(event.target.value);
          }}
        />
        <span className="text-sm font-normal text-muted-foreground">
          The first line becomes the title of the pull request.
        </span>
      </label>
      <DiffText lines={changes} errorLine={null} isChangesOnly />
      {problem && (
        <Alert variant="destructive">
          <WarningIcon />
          <AlertContent>
            <AlertDescription>{problem.detail}</AlertDescription>
          </AlertContent>
        </Alert>
      )}
      <ModalFooter>
        <Button onClick={onClose}>Cancel</Button>
        <Button
          variant="primary"
          isPending={propose.isPending}
          pendingText="Opening"
          disabled={message.trim() === "" || !config.sha}
          onClick={() => {
            propose.mutate({
              path: { org, repo },
              body: {
                branch: config.branch,
                base_sha: config.sha ?? "",
                document: completeDocument(draft.document),
                message,
              },
            });
          }}
        >
          Open the pull request
        </Button>
      </ModalFooter>
    </Modal>
  );
}
