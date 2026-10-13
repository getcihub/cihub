/** A sentence of the API, where a check or a condition is a Markdown code span. */
export function Prose({ text }: { text: string }) {
  return (
    <>
      {text.split("`").map((part, index) =>
        // The odd parts are inside the backticks; the parts of one sentence never move.
        index % 2 === 1 ? <code key={index}>{part}</code> : part,
      )}
    </>
  );
}
