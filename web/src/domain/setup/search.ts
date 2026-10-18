import { z } from "zod";

/**
 * The search of the page of an organization: `connecting` after the Setup URL of the App, `q` to search the
 * repositories. `/orgs/$org/repos` takes the same and leads here.
 */
export const orgSearch = z.object({
  connecting: z.enum(["install", "update"]).optional().catch(undefined),
  // An empty search is no search.
  q: z.string().min(1).optional().catch(undefined),
});
