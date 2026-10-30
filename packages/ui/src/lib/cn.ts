import { type ClassValue, clsx } from "clsx";
import { twMerge } from "tailwind-merge";

/** The classes of `inputs`, joined; of two classes that set the same property, the last one wins. */
export function cn(...inputs: ClassValue[]): string {
  return twMerge(clsx(inputs));
}
