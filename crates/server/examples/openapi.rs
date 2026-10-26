//! Writes the `OpenAPI` document of the API v1 on the standard output, as the
//! server serves it on `/api/v1/openapi.json`, without a database: the client
//! of the dashboard is generated from it (`mise run web:generate`).

fn main() -> Result<(), serde_json::Error> {
    // The modules that `server::merge_queue` mounts: a new module adds its
    // document here too, or `schema/openapi/v1.json` misses its routes.
    let document = cihub_web::openapi(&[cihub_mergequeue::openapi()]);
    let json = serde_json::to_string_pretty(&document)?;

    #[expect(
        clippy::print_stdout,
        reason = "the document goes to the standard output, for a file"
    )]
    {
        println!("{json}");
    }
    Ok(())
}
