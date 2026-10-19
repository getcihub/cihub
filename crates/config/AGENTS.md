# cihub-config

- A strategy is a row of default values in the strategy table, not code in `cihub-mq-core`.
- A new key of `.cihub.yaml` is documented in `specs/CONFIGURATION.md` and described in `schema/config/v1.json`
  in the same change; `tests/schema.rs` fails until the schema follows the parser.
