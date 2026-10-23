import type { ConfigSettings, MergeMethod, QueueMode, SettingSource, Strategy } from "@/client/types.gen";

type Trigger = ConfigSettings["trigger"];
type MergeCommitTitle = ConfigSettings["merge_commit_title"];
type BranchProtectionInjection = ConfigSettings["branch_protection_injection"];

// The words of the builder of the mockups v26: the client words a value of the API, the server gives the value.

/** A value and its words: what the picker says, and the description under it. */
export interface ValueWords<T extends string> {
  value: T;
  label: string;
  hint?: string;
}

/** The strategies, as their cards say them. The server refuses `parallel` until it is ready. */
export const STRATEGIES: { value: Strategy; title: string; text: string; isReady: boolean }[] = [
  {
    value: "sequential",
    title: "Sequential",
    text: "One run at a time, each pull request alone.",
    isReady: true,
  },
  {
    value: "pipelined",
    title: "Pipelined",
    text: "Several runs at once, each one on top of the runs before it.",
    isReady: true,
  },
  {
    value: "batched",
    title: "Batched",
    text: "Several pull requests in one run, for a slow or costly CI.",
    isReady: true,
  },
  {
    value: "parallel",
    title: "Parallel",
    text: "Batches in independent lines, for unrelated changes.",
    isReady: false,
  },
];

export const MODES: ValueWords<QueueMode>[] = [
  { value: "single", label: "one line", hint: "Each run on top of the runs before it." },
  { value: "parallel", label: "parallel lines", hint: "Unrelated changes test apart." },
];

export const WINDOW_KINDS: ValueWords<"adaptive" | "fixed">[] = [
  {
    value: "adaptive",
    label: "adaptive",
    hint: "Between a floor and a ceiling: CIHub lowers it after failures.",
  },
  { value: "fixed", label: "fixed", hint: "Always the same number of runs." },
];

export const TRIGGERS: ValueWords<Exclude<Trigger, "inplace">>[] = [
  {
    value: "draft_pr",
    label: "a draft pull request",
    hint: "CIHub opens a draft pull request for each test.",
  },
  { value: "push", label: "a push", hint: "CIHub pushes the test branch, and your CI runs on the push." },
];

/** The merge methods; fast-forward only with the sequential strategy, and never in a profile. */
export function mergeMethods(isSequential: boolean, withFastForward = true): ValueWords<MergeMethod>[] {
  const methods: (ValueWords<MergeMethod> & { disabled?: boolean })[] = [
    { value: "squash", label: "squash", hint: "One commit per pull request." },
    {
      value: "merge_commit",
      label: "merge commit",
      hint: "The commits of the pull request, and a merge commit.",
    },
    { value: "rebase", label: "rebase", hint: "The commits of the pull request, one after the other." },
  ];
  if (withFastForward) {
    methods.push({
      value: "fast_forward",
      label: "fast forward",
      hint: isSequential ? "The tested commit lands as is." : "Needs the sequential strategy.",
      disabled: !isSequential,
    });
  }
  return methods;
}

export const MERGE_TITLES: ValueWords<MergeCommitTitle>[] = [
  { value: "pr_title", label: "the title", hint: "The title of the pull request." },
  {
    value: "pr_title_and_number",
    label: "the title and the number",
    hint: "The title, then the number: “Fix the login (#121)”.",
  },
];

export const INJECTIONS: ValueWords<BranchProtectionInjection>[] = [
  {
    value: "both",
    label: "before the admission and on the test",
    hint: "A pull request waits for them to enter, and the tested commit needs them.",
  },
  { value: "merge", label: "on the test only", hint: "Only the tested commit needs them." },
  { value: "none", label: "nowhere", hint: "CIHub does not count them." },
];

export const PRIORITIES: ValueWords<string>[] = [
  { value: "urgent", label: "urgent", hint: "0: ahead of every other pull request" },
  { value: "high", label: "high", hint: "10" },
  { value: "medium", label: "medium", hint: "100: without a rule" },
  { value: "low", label: "low", hint: "200: after the others" },
];

/** The words of `value` among `words`; the value itself for one that they do not have. */
export function wordsOf<T extends string>(words: ValueWords<T>[], value: T): string {
  return words.find((word) => word.value === value)?.label ?? value;
}

/** Who sets a value: the part of the configuration that the person looks at, or a weaker one that it takes. */
export type Owner = "defaults" | "queue" | "profile";

/** Where a value comes from, said from the part that shows it. */
export function sourceWords(source: SettingSource, owner: Owner): string {
  if (source === owner) {
    return { defaults: "in defaults", queue: "in this queue", profile: "in this profile" }[owner];
  }
  return {
    default: "default",
    strategy: "from the strategy",
    defaults: "from defaults",
    queue: "from the queue",
    profile: "in this profile",
  }[source];
}
