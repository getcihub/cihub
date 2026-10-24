/** The indent of a line: the spaces before its text. */
function indentOf(line: string): number {
  return line.length - line.trimStart().length;
}

/**
 * The lines of the key `key` of a part of a YAML file, the key first, at the indent of the key: under `defaults`,
 * or under the entry of `queues` of the branch `entry`. The file as CIHub writes it, or as a person wrote it;
 * none when the part does not set the key.
 */
export function yamlBlockOf(text: string, entry: string | null, key: string): string[] {
  const lines = text.split("\n");
  const isEntry = (line: string) => {
    const match = /^ {2}- branch: (.*)$/.exec(line);
    return match?.[1]?.trim().replace(/^["']|["']$/g, "") === entry;
  };
  const start = entry === null ? lines.findIndex((line) => line === "defaults:") : lines.findIndex(isEntry);
  if (start < 0) {
    return [];
  }

  // The keys of the part: those of `defaults` at 2 spaces, those of an entry at 4, until the part ends.
  const indent = entry === null ? 2 : 4;
  let found = -1;
  for (let index = start + 1; index < lines.length; index += 1) {
    const line = lines[index] ?? "";
    if (line.trim() !== "" && indentOf(line) < indent) {
      break;
    }
    if (indentOf(line) === indent && line.trimStart().startsWith(`${key}:`)) {
      found = index;
      break;
    }
  }
  if (found < 0) {
    return [];
  }

  const block = [lines[found] ?? ""];
  for (let index = found + 1; index < lines.length; index += 1) {
    const line = lines[index] ?? "";
    if (line.trim() !== "" && indentOf(line) <= indent) {
      break;
    }
    block.push(line);
  }
  return block.filter((line) => line.trim() !== "").map((line) => line.slice(indent));
}
