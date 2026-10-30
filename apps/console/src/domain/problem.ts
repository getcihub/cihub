import type { ErrorCode, Problem } from "@/client/types.gen";

/** Why a request failed: the problem that the API answered, or the error of a network that did not answer. */
export type Failure = Problem | Error;

/**
 * What a route threw, as a failure. The router types it `unknown`, as a script can throw anything; the reads of
 * the dashboard throw the problem of the API, as the generated client declares, and the rest an `Error`.
 */
export function failureOf(thrown: unknown): Failure {
  return thrown instanceof Error ? thrown : (thrown as Problem);
}

/** The problem that the API answered; `null` when it did not answer. */
export function problemOf(failure: Failure): Problem | null {
  return failure instanceof Error ? null : failure;
}

/** Whether `failure` is a problem of the API with the code `code`. */
export function isProblem(failure: Failure, code: ErrorCode): boolean {
  return problemOf(failure)?.code === code;
}

/** What the API answered to a refused action: its problem, else that it did not answer. */
export function refusalOf(failure: Failure): string {
  const problem = problemOf(failure);
  if (!problem) {
    return "The server did not answer. Try again.";
  }

  return problem.detail || "The server refused the request.";
}
