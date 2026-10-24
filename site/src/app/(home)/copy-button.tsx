"use client";

import { useCopyButton } from "fumadocs-ui/utils/use-copy-button";

import { CheckIcon, CopyIcon } from "./icons";

export function CopyButton({ text }: { text: string }) {
  const [isChecked, onClick] = useCopyButton(() => navigator.clipboard.writeText(text));

  return (
    <button type="button" className={isChecked ? "copy done" : "copy"} onClick={onClick} aria-label="Copy">
      {isChecked ? <CheckIcon /> : <CopyIcon />}
    </button>
  );
}
