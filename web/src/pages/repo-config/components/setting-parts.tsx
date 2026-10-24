import type { ReactNode } from "react";

import type { SettingSource } from "@/client/types.gen";
import { Card } from "@/components/ui/card";
import { type Owner, sourceWords } from "@/domain/config/words";
import { cn } from "@/lib/cn";
import { type Editor, type RowKeys, isChanged, isSet } from "@/pages/repo-config/setting-editor";

/** A part of the builder: its title and what it covers on the left, its settings on the right. */
export function SettingsSection({
  id,
  title,
  text,
  children,
}: {
  id: string;
  title: string;
  text: string;
  children: ReactNode;
}) {
  return (
    <section
      id={id}
      aria-label={title}
      className="grid scroll-mt-17 gap-3 border-t border-divider py-5.5 first-of-type:border-t-0 first-of-type:pt-1.5 lg:grid-cols-[280px_minmax(0,1fr)] lg:gap-8"
    >
      <div>
        <h3 className="mb-1 text-xl font-medium">{title}</h3>
        <p className="text-base text-secondary-foreground">{text}</p>
      </div>
      <div className="grid min-w-0 content-start gap-2">{children}</div>
    </section>
  );
}

/** A row of the builder for the keys of `row` in the part of `editor`: its tag, and "use the default". */
export function Setting({
  editor,
  row,
  title,
  text,
  children,
  isColumn,
}: {
  editor: Editor;
  row: RowKeys;
  title: string;
  text: ReactNode;
  children: ReactNode;
  isColumn?: boolean;
}) {
  const isOwn = row.keys.some((key) => isSet(editor, key));

  return (
    <SettingRow
      title={title}
      tag={
        <SourceTag
          source={editor.resolved.sources[row.source]}
          owner={editor.owner}
          isChanged={row.keys.some((key) => isChanged(editor, key))}
          onReset={
            editor.canEdit && isOwn
              ? () => {
                  editor.set(Object.fromEntries(row.keys.map((key) => [key, null])));
                }
              : undefined
          }
        />
      }
      text={text}
      isColumn={isColumn}
    >
      {children}
    </SettingRow>
  );
}

/** A setting: its name and where its value comes from, what it does, then its value on the right. */
export function SettingRow({
  title,
  tag,
  text,
  children,
  isColumn = false,
}: {
  title: string;
  tag?: ReactNode;
  text: ReactNode;
  children?: ReactNode;
  /** The value goes under the text, for a value too wide for the right. */
  isColumn?: boolean;
}) {
  return (
    <Card
      className={cn(
        "flex gap-5 px-4 py-3.5 max-md:flex-col max-md:items-stretch max-md:gap-2.5",
        isColumn ? "flex-col items-stretch gap-3" : "items-center",
      )}
    >
      <div className="grid min-w-0 flex-1 gap-0.5">
        <b className="flex flex-wrap items-center gap-2 font-medium">
          <span>{title}</span>
          {tag}
        </b>
        <span className="text-md text-secondary-foreground">{text}</span>
      </div>
      {children !== undefined && (
        <div
          className={cn(
            isColumn
              ? "grid gap-2"
              : "flex flex-none flex-wrap items-center justify-end gap-2 max-md:justify-start md:max-w-[55%]",
          )}
        >
          {children}
        </div>
      )}
    </Card>
  );
}

/**
 * Where a value comes from: the part that the person looks at sets it, in the color of the primary; a weaker
 * part of the file gives it, dashed; a change of the draft, in the color of a warning. "use the default" leaves
 * the value to a weaker source.
 */
export function SourceTag({
  source,
  owner,
  isChanged = false,
  onReset,
}: {
  source: SettingSource;
  owner: Owner;
  isChanged?: boolean;
  onReset?: (() => void) | undefined;
}) {
  const isInherited = source !== owner && (source === "defaults" || source === "queue");

  return (
    <>
      <span
        data-slot="source-tag"
        className={cn(
          "rounded-xs border px-1.25 text-2xs font-normal whitespace-nowrap",
          isChanged
            ? "border-warning-border text-warning"
            : source === owner
              ? "border-primary-border text-primary"
              : isInherited
                ? "border-dashed border-border text-secondary-foreground"
                : "border-border text-muted-foreground",
        )}
      >
        {isChanged ? "changed" : sourceWords(source, owner)}
      </span>
      {onReset && (
        <button
          type="button"
          className="cursor-pointer text-xs font-normal text-primary hover:underline"
          onClick={onReset}
        >
          {owner === "defaults" ? "use the value of CIHub" : "use the default"}
        </button>
      )}
    </>
  );
}

/** Names of checks or labels, each in code. */
export function Names({ names }: { names: string[] }) {
  return (
    <span className="inline-flex flex-wrap gap-1.5">
      {names.map((name) => (
        <code key={name} className="border border-border">
          {name}
        </code>
      ))}
    </span>
  );
}
