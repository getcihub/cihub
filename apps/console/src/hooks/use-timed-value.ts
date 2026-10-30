import { useEffect, useState } from "react";

/**
 * A value that the page shows for `shownMs`, then forgets: what a copy or an action did. A new value shows for
 * `shownMs` again.
 */
export function useTimedValue<Value>(shownMs: number): [Value | null, (value: Value | null) => void] {
  const [value, setValue] = useState<Value | null>(null);

  useEffect(() => {
    if (value === null) {
      return;
    }

    const timer = setTimeout(() => {
      setValue(null);
    }, shownMs);

    return () => {
      clearTimeout(timer);
    };
  }, [value, shownMs]);

  return [value, setValue];
}
