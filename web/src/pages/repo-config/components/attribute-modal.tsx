import { useState } from "react";

import { Button } from "@/components/ui/button";
import { Modal, ModalFooter } from "@/components/ui/modal";
import { ATTRIBUTE_CATALOG } from "@/domain/config/conditions";

/** The catalog of the attributes, by subject, with a search: a click chooses one. */
export function AttributeModal({
  onClose,
  onChoose,
}: {
  onClose: () => void;
  onChoose: (attribute: string) => void;
}) {
  const [query, setQuery] = useState("");
  const text = query.trim().toLowerCase();
  const found = Object.entries(ATTRIBUTE_CATALOG).filter(([name, words]) =>
    `${name} ${words.label} ${words.text} ${words.group}`.toLowerCase().includes(text),
  );
  const groups = [...new Set(found.map(([, words]) => words.group))];

  return (
    <Modal title="Choose an attribute" onClose={onClose}>
      <input
        autoFocus
        className="h-8 border-b border-divider bg-transparent text-xl outline-none"
        placeholder="Search the attributes: label, author, check…"
        aria-label="Search the attributes"
        value={query}
        onChange={(event) => {
          setQuery(event.target.value);
        }}
      />
      <div className="-mx-2 grid max-h-95 gap-0.5 overflow-y-auto">
        {found.length === 0 && <p className="p-2 text-md text-muted-foreground">No attribute matches.</p>}
        {groups.map((group) => (
          <div key={group} className="grid gap-0.5">
            <div className="mx-2 mt-2.5 mb-1 text-xs tracking-wide text-muted-foreground uppercase">
              {group}
            </div>
            {found
              .filter(([, words]) => words.group === group)
              .map(([name, words]) => (
                <button
                  key={name}
                  type="button"
                  className="grid cursor-pointer justify-items-start gap-0.5 rounded-sm p-2 text-left hover:bg-muted"
                  onClick={() => {
                    onChoose(name);
                  }}
                >
                  <b className="font-medium">
                    {words.label} <code className="text-primary">{name}</code>
                  </b>
                  <span className="text-md text-secondary-foreground">{words.text}</span>
                </button>
              ))}
          </div>
        ))}
      </div>
      <ModalFooter className="items-center justify-between">
        <span className="text-md text-muted-foreground">
          Each attribute reads one fact of the pull request.
        </span>
        <Button onClick={onClose}>Cancel</Button>
      </ModalFooter>
    </Modal>
  );
}
