# @cihub/utils

The functions without React and without CSS that more than one package needs. A function that only one app needs
stays in that app (`apps/console/src/lib/`).

- `src/utc-days.ts`: hours, days and months in UTC as milliseconds, for `RangeCalendar` of `@cihub/ui` and the
  periods of the Health of the console.
- One file per subject, imported by its path: `@cihub/utils/utc-days`. No barrel file.

`mise run packages:lint` type checks, lints and checks the format.
