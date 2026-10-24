import { getBranchConfigOptions } from "@/client/@tanstack/react-query.gen";

/** The read of the configuration file of the default branch of `repo`, with what it sets. */
export function branchConfigOptions(org: string, repo: string): ReturnType<typeof getBranchConfigOptions> {
  return getBranchConfigOptions({ path: { org, repo } });
}

/** The entry of the builder for `defaults`; an entry of a queue is its `branch`, which never takes this name. */
export const DEFAULTS_ENTRY = "defaults";

/** The views of the configuration: the builder, or the file as it is. */
export const VIEWS = ["builder", "yaml"] as const;
export type View = (typeof VIEWS)[number];
