import { act, renderHook } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { useReadAgain } from "./use-read-again";

const queryOf = (data: boolean | undefined) => ({ state: { data } });
const isWaiting = (data: boolean | undefined) => data === true;

afterEach(() => {
  vi.useRealTimers();
});

describe("the wait of a page for a read of the server", () => {
  it("starts when an answer begins to wait, however long the page has been open", () => {
    vi.useFakeTimers();
    const { result } = renderHook(() => useReadAgain(isWaiting));

    expect(result.current.refetchInterval(queryOf(false))).toBe(false);
    expect(result.current.hasStarted).toBe(false);

    act(() => {
      vi.advanceTimersByTime(90_000);
    });
    act(() => {
      result.current.refetchInterval(queryOf(true));
    });

    expect(result.current.hasStarted).toBe(true);
    expect(result.current.isOver).toBe(false);
    expect(result.current.refetchInterval(queryOf(true))).toBe(2000);

    act(() => {
      vi.advanceTimersByTime(60_000);
    });

    expect(result.current.isOver).toBe(true);
    expect(result.current.refetchInterval(queryOf(true))).toBe(false);
  });

  it("waits again from an answer that waits after one that did not", () => {
    vi.useFakeTimers();
    const { result } = renderHook(() => useReadAgain(isWaiting));

    act(() => {
      result.current.refetchInterval(queryOf(true));
    });
    act(() => {
      vi.advanceTimersByTime(60_000);
    });

    expect(result.current.isOver).toBe(true);

    act(() => {
      result.current.refetchInterval(queryOf(false));
      result.current.refetchInterval(queryOf(true));
    });

    expect(result.current.isOver).toBe(false);
    expect(result.current.refetchInterval(queryOf(true))).toBe(2000);
  });

  it("waits only from a restart when the data does not start it", () => {
    vi.useFakeTimers();
    const { result } = renderHook(() => useReadAgain(isWaiting, false));

    expect(result.current.refetchInterval(queryOf(true))).toBe(false);
    expect(result.current.hasStarted).toBe(false);

    act(() => {
      result.current.restart();
    });

    expect(result.current.hasStarted).toBe(true);
    expect(result.current.refetchInterval(queryOf(true))).toBe(2000);
  });
});
