import { keepPreviousData, useQuery } from "@tanstack/react-query";
import { useEffect, useState } from "react";

import { previewConfig } from "@/client/sdk.gen";
import type { BranchConfig, ConfigDocument, ConfigPreview, ResolvedConfig } from "@/client/types.gen";
import { EMPTY_DOCUMENT, changed, completeDocument, isSameDocument } from "@/domain/config/document";

/** How long the builder waits after the last change before it asks the server for the file. */
const PREVIEW_AFTER_MS = 100;

export interface ConfigDraft {
  /** The document of the file, as the server read it; an empty one for a file that is not valid. */
  base: ConfigDocument;
  /** The document that the builder shows: the draft of the person, else the file. */
  document: ConfigDocument;
  /** The person changed the document: a change to propose. */
  isChanged: boolean;
  change: (change: (copy: ConfigDocument) => void) => void;
  undo: () => void;
  /** The file of the last draft that the server wrote, with its error, its diff and its settings. */
  preview: ConfigPreview | null;
  /** The settings that the builder shows: those of the preview of the draft, else those of the file. */
  settings: ResolvedConfig | null;
}

/**
 * The draft of the file of `config`, which the person changes in the builder: the server writes its YAML, checks
 * it and gives its settings 100 ms after the last change. A file that is not valid starts a draft from an empty
 * document for a person who can propose: writing the file again fixes it.
 */
export function useConfigDraft(
  org: string,
  repo: string,
  config: BranchConfig,
  canEdit: boolean,
): ConfigDraft {
  const base = config.document ?? EMPTY_DOCUMENT;
  const [draft, setDraft] = useState<ConfigDocument | null>(
    config.document === null && canEdit ? EMPTY_DOCUMENT : null,
  );
  // The draft that the server writes: the last one that stayed 100 ms.
  const [sent, setSent] = useState<ConfigDocument | null>(draft);
  useEffect(() => {
    const timer = setTimeout(() => {
      setSent(draft);
    }, PREVIEW_AFTER_MS);
    return () => {
      clearTimeout(timer);
    };
  }, [draft]);
  const { data: preview } = useQuery({
    queryKey: ["previewConfig", org, repo, config.branch, sent],
    queryFn: async ({ signal }) => {
      const { data } = await previewConfig({
        path: { org, repo },
        body: { branch: config.branch, document: completeDocument(sent ?? EMPTY_DOCUMENT) },
        signal,
        throwOnError: true,
      });
      return data;
    },
    enabled: sent !== null,
    // The builder keeps the settings of the last answer while the next one comes.
    placeholderData: keepPreviousData,
  });

  const document = draft ?? base;
  const isChanged = draft !== null;

  return {
    base,
    document,
    isChanged,
    change: (change) => {
      setDraft((previous) => {
        const next = changed(previous ?? base, change);
        return config.document !== null && isSameDocument(next, base) ? null : next;
      });
    },
    undo: () => {
      setDraft(config.document === null && canEdit ? EMPTY_DOCUMENT : null);
    },
    preview: isChanged ? (preview ?? null) : null,
    settings: (isChanged ? preview?.settings : null) ?? config.settings ?? null,
  };
}
