import { type Failure, refusalOf } from "@/domain/problem";

/** Why the server refused an action, next to its button; nothing while the action did not fail. */
export function MutationError({ error }: { error: Failure | null }) {
  if (!error) return null;

  return (
    <p className="text-md text-destructive" role="alert">
      {refusalOf(error)}
    </p>
  );
}
