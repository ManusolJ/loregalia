use loregalia_core::providers::tvmaze;
use std::path::Path;

fn main() -> loregalia_core::Result<()> {
    let mut arguments = std::env::args().skip(1);
    let query = arguments.next().unwrap_or_else(|| "a".to_owned());
    let file = arguments
        .next()
        .unwrap_or_else(|| "search-a.json".to_owned());

    let body = tvmaze::search_json(&query)?;
    let shows = tvmaze::parse_search(&body)?;
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tvmaze");
    std::fs::create_dir_all(&directory)?;
    let path = directory.join(file);
    std::fs::write(&path, body)?;

    println!("wrote {} shows to {}", shows.len(), path.display());

    Ok(())
}
