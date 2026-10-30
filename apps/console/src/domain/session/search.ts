import { z } from "zod";

/**
 * A path of this site, to open after the sign-in: never another site, as the server checks it too (`page_after` in
 * `crates/web/src/sign_in.rs`).
 */
export const pageOfThisSite = z.string().regex(/^\/(?!\/)[A-Za-z0-9/_.~?=&%-]*$/, "a path of this site");

/** The search of the sign-in page: a sign-in that failed, and the page to open after it. */
export const signInSearch = z.object({
  error: z.literal("failed").optional().catch(undefined),
  next: pageOfThisSite.optional().catch(undefined),
});
