/** A page that waits for a read of the server reads again this often… */
const READ_AGAIN_EVERY_MS = 2000;
/** …for this long at most after the wait started: past it, the person reloads or checks again. */
export const READ_AGAIN_FOR_MS = 60_000;

/**
 * The `refetchInterval` of a read that a page follows while it waits: every 2 seconds while `isWaiting` says so
 * of its last answer, for 60 seconds at most after `since`. The dashboard polls nothing else but the Health; the
 * stream says the rest (`specs/LOTS.md`, lot 16).
 */
export function readAgainWhile<T>(
  isWaiting: (data: T | undefined) => boolean,
  since: number,
): (query: { state: { data: T | undefined } }) => number | false {
  return (query) =>
    isWaiting(query.state.data) && Date.now() - since < READ_AGAIN_FOR_MS ? READ_AGAIN_EVERY_MS : false;
}
