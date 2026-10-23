import type {
  ConfigDocument,
  ConfigSettings,
  DiffLine,
  QueueDocument,
  RequiredStatusesDocument,
  ResolvedConfig,
  SettingSource,
  SettingSources,
  SettingsDocument,
} from "@/client/types.gen";
import { completeConditions } from "./conditions";
import { DEFAULTS_ENTRY } from "./queries";

/** A document without a key: the file that sets nothing. */
export const EMPTY_DOCUMENT: ConfigDocument = {
  defaults: { settings: {}, queue_conditions: [], merge_checks: [] },
  queues: [],
  priority_rules: [],
};

/** The part of a document that the builder shows: `defaults`, or the entry of a queue, by its index. */
export type Item = { kind: "defaults" } | { kind: "queue"; index: number };

/** The item of `entry`, the name of an entry in the address: `defaults`, or the `branch` of a queue. */
export function itemOf(document: ConfigDocument, entry: string): Item | null {
  if (entry === DEFAULTS_ENTRY) {
    return { kind: "defaults" };
  }

  const index = document.queues.findIndex((queue) => queue.branch === entry);
  return index < 0 ? null : { kind: "queue", index };
}

/** The settings that `item` sets in `document`. */
export function settingsOf(document: ConfigDocument, item: Item): SettingsDocument {
  return item.kind === "defaults"
    ? document.defaults.settings
    : (document.queues[item.index]?.settings ?? {});
}

/** The settings that follow for `item`, with their sources; those of `defaults` while a new entry waits for them. */
export function resolvedOf(resolved: ResolvedConfig, item: Item): ConfigSettings {
  return item.kind === "defaults"
    ? resolved.defaults
    : (resolved.queues[item.index]?.settings ?? resolved.defaults);
}

/**
 * `resolved` with what `own` sets, from `source`: a control shows its new value at once, without waiting for the
 * preview of the server, which gives what follows from it (the values of a strategy, of `defaults`).
 */
export function withOwnSettings(
  resolved: ConfigSettings,
  own: SettingsDocument,
  source: SettingSource,
): ConfigSettings {
  const next: ConfigSettings = { ...resolved, sources: { ...resolved.sources } };
  const values: Record<string, unknown> = next;
  for (const [key, value] of Object.entries(own)) {
    if (value === null) {
      continue;
    }
    if (key === "testing_concurrency") {
      next.window_floor = next.window_ceiling = value as number;
      next.sources.window_floor = next.sources.window_ceiling = source;
    } else if (key === "required_statuses") {
      const required = value as RequiredStatusesDocument;
      next.required_statuses = required.kind === "checks" ? required.names : null;
      next.sources.required_statuses = source;
    } else if (key in next.sources) {
      values[key] = value;
      next.sources[key as keyof SettingSources] = source;
    }
  }
  return next;
}

/** `document` with the change of `change`, made on a copy: the document of the server never changes. */
export function changed(document: ConfigDocument, change: (copy: ConfigDocument) => void): ConfigDocument {
  const copy = structuredClone(document);
  change(copy);
  return copy;
}

/** The settings of `item` in `copy`, to change them. */
export function settingsIn(copy: ConfigDocument, item: Item): SettingsDocument {
  if (item.kind === "defaults") {
    return copy.defaults.settings;
  }
  const queue = copy.queues[item.index];
  if (!queue) {
    throw new Error(`no entry ${String(item.index)}`);
  }
  return queue.settings;
}

/** A new entry of `branch`, which sets nothing: it takes `defaults`. */
export function newQueue(branch: string): QueueDocument {
  return { branch, settings: {}, queue_conditions: [], merge_checks: [], profiles: [] };
}

/** A branch name that is a glob: it holds one of `*?[`, which git refuses in a name. */
export function isGlob(branch: string): boolean {
  return /[*?[]/.test(branch);
}

/** Two documents set the same: the draft has no change left. A key set to `null` is a key that is not set. */
export function isSameDocument(first: ConfigDocument, second: ConfigDocument): boolean {
  const withoutNulls = (_: string, value: unknown) => (value === null ? undefined : value);
  return JSON.stringify(first, withoutNulls) === JSON.stringify(second, withoutNulls);
}

/** How many lines a diff adds and removes. */
export function countChanges(diff: DiffLine[]): { added: number; removed: number } {
  return {
    added: diff.filter((line) => line.change === "added").length,
    removed: diff.filter((line) => line.change === "removed").length,
  };
}

/**
 * What the file can hold of `document`: a condition that is not complete, and a priority rule without a
 * condition, stay in the draft, out of the file.
 */
export function completeDocument(document: ConfigDocument): ConfigDocument {
  return changed(document, (copy) => {
    copy.defaults.queue_conditions = completeConditions(copy.defaults.queue_conditions);
    for (const queue of copy.queues) {
      queue.queue_conditions = completeConditions(queue.queue_conditions);
      for (const profile of queue.profiles) {
        profile.conditions = completeConditions(profile.conditions);
      }
    }
    copy.priority_rules = copy.priority_rules
      .map((rule) => ({ ...rule, conditions: completeConditions(rule.conditions) }))
      .filter((rule) => rule.conditions.length > 0);
  });
}
