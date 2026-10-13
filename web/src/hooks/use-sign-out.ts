import { useState } from "react";

import { signOut } from "@/domain/session/session";

export interface SignOut {
  /** Ends the session, then opens the sign-in page. */
  startSignOut: () => void;
  /** The server did not end the session: the old one stays valid, and the page says so. */
  hasSignOutFailed: boolean;
}

/**
 * The sign-out of a page, and whether it failed. Once the server ends the session, a full load of the sign-in
 * page, and not a navigation of the router, clears the data of the old session from the cache of the queries.
 */
export function useSignOut(): SignOut {
  const [hasSignOutFailed, setHasSignOutFailed] = useState(false);

  const startSignOut = () => {
    void signOut().then((hasSignedOut) => {
      if (hasSignedOut) {
        window.location.assign("/sign-in");
        return;
      }
      setHasSignOutFailed(true);
    });
  };

  return { startSignOut, hasSignOutFailed };
}
