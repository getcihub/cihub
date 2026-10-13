/** `count` with the word that agrees with it: "1 run", "3 runs". */
export function plural(count: number, one: string, many: string): string {
  return `${String(count)} ${count === 1 ? one : many}`;
}

/** The first 7 characters of a commit, as a person reads it. */
export function shortSha(sha: string): string {
  return sha.slice(0, 7);
}
