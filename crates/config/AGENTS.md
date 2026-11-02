# cihub-config

- A strategy is a row of default values in the strategy table, not code in `cihub-mq-core`.
- Each key of `.cihub.yaml` has a `///` comment and an entry in `schema/config/v1.json`, in the same change;
  `tests/schema.rs` fails until the schema follows the parser. `specs/CONFIGURATION.md` states only the rules that
  the code cannot show.
