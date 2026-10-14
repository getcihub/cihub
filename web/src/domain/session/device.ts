// The sign-in of the command line (`specs/API.md`): `cihub auth login` shows a code, and the page `/device`
// approves it with the session of the person.

const LETTERS = /^[BCDFGHJKLMNPQRSTVWXZ]{8}$/;

/**
 * The code that a person typed, as the command line shows it (`WDJB-MJHT`): the case, the spaces and the dash do
 * not matter, as for the server (`parse_user_code`). `null` when it cannot be a code.
 */
export function userCodeOf(typed: string): string | null {
  const letters = typed.replace(/[\s-]/g, "").toUpperCase();
  return LETTERS.test(letters) ? `${letters.slice(0, 4)}-${letters.slice(4)}` : null;
}

/** The page that approves `code`, to open after the sign-in. */
export function deviceAddress(code: string | undefined): string {
  return code ? `/device?code=${encodeURIComponent(code)}` : "/device";
}
