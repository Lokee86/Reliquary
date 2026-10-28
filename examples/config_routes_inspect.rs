use reliquary_memory::ReliquaryConfig;
use std::path::Path;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let p = std::env::args().nth(1).expect("config");
    let c = ReliquaryConfig::open(Path::new(&p))?;
    for (name, route) in [
        ("general", c.models.general.as_ref()),
        ("insomnia", c.models.insomnia.as_ref()),
        ("insomnia_metadata", c.models.insomnia_metadata.as_ref()),
        ("entity_extraction", c.models.entity_extraction.as_ref()),
        ("entity_resolution", c.models.entity_resolution.as_ref()),
        ("dream", c.models.dream.as_ref()),
    ] {
        match route {
            Some(r) => println!(
                "{name}: provider={:?} model={} reasoning={:?} credential={:?}",
                r.provider, r.model, r.reasoning_effort, r.credential_id
            ),
            None => println!("{name}: <none>"),
        }
    }
    Ok(())
}
