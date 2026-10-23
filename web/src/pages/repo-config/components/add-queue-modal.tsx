import { useState } from "react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Modal, ModalDescription, ModalFooter } from "@/components/ui/modal";

/** The branch of a new entry: one branch, or a glob that gives a queue to each branch that it matches. */
export function AddQueueModal({
  branch,
  entries,
  onClose,
  onAdd,
}: {
  branch: string;
  entries: string[];
  onClose: () => void;
  onAdd: (entry: string) => void;
}) {
  const [entry, setEntry] = useState("");
  const name = entry.trim();
  const isTaken = entries.includes(name);

  return (
    <Modal title="Add a queue" onClose={onClose}>
      <ModalDescription>
        A branch, or a glob such as <code>release/*</code>: each branch that it matches gets its own queue.
        CIHub reads the file at the head of each branch, so the entry acts on a branch once the file is on it,
        as on <code>{branch}</code>.
      </ModalDescription>
      <label className="grid gap-1 text-md font-medium">
        Branch
        <Input
          autoFocus
          value={entry}
          placeholder="release/*"
          onChange={(event) => {
            setEntry(event.target.value);
          }}
        />
      </label>
      {isTaken && <p className="text-md text-destructive">The file already has an entry of this branch.</p>}
      <ModalFooter>
        <Button onClick={onClose}>Cancel</Button>
        <Button
          variant="primary"
          disabled={name === "" || isTaken}
          onClick={() => {
            onAdd(name);
          }}
        >
          Add the queue
        </Button>
      </ModalFooter>
    </Modal>
  );
}
