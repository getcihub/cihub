import type { ErrorCode, Problem } from "@/client/types.gen";

/** Why a request failed: the problem that the API answered, or the error of a network that did not answer. */
export type Failure = Problem | Error;

// The router and a rejected promise type what they hold `unknown`: a script can throw anything. This is the one
// place that turns it into a failure.
// eslint-disable-next-line no-restricted-syntax -- a thrown value has no type
function isApiProblem(thrown: unknown): thrown is Problem {
  return typeof thrown === "object" && thrown !== null && "code" in thrown && "status" in thrown;
}

/**
 * What a route threw, as a failure: the problem of the API, as the generated client throws it, an `Error`, or else
 * an `Error` that says that the page failed.
 */
// eslint-disable-next-line no-restricted-syntax -- a thrown value has no type
export function failureOf(thrown: unknown): Failure {
  if (thrown instanceof Error || isApiProblem(thrown)) {
    return thrown;
  }

  return new Error("The page failed.");
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
