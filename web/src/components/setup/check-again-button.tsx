import { Button } from "@/components/ui/button";

/** "Check again", for an admin: the server reads the setup again; the button runs while the page waits for it. */
export function CheckAgainButton({
  checkAgain,
  isChecking,
  hasGivenUp,
}: {
  checkAgain: { run: () => void; isPending: boolean };
  isChecking: boolean;
  hasGivenUp: boolean;
}) {
  return (
    <Button
      isPending={checkAgain.isPending || (isChecking && !hasGivenUp)}
      pendingText="Checking"
      onClick={checkAgain.run}
    >
      Check again
    </Button>
  );
}
