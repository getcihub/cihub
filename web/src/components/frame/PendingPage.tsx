import { Page } from "@/ui/page";
import { LoadingStatus } from "@/ui/skeleton";
import { TopBar, TopBarLogo } from "./Frame";

/** A page outside an organization reads its data (the sign-in, `/`): the top bar, empty, so that it does not jump. */
export function PendingPage() {
  return (
    <>
      <TopBar>
        <TopBarLogo />
      </TopBar>
      <Page aria-busy="true">
        <LoadingStatus />
      </Page>
    </>
  );
}
