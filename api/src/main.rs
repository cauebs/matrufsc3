use std::fs::File;

use anyhow::Result;

use scraper::{Campus, scrape_last_n_terms};

#[tokio::main]
async fn main() -> Result<()> {
    let selected_campi = [
        Campus::FLO,
        Campus::JOI,
        Campus::CBS,
        Campus::ARA,
        Campus::BLN,
    ];

    let datasets = scrape_last_n_terms(3, &selected_campi).await;

    for dataset in datasets {
        let (campus, term, classes) = dataset?;

        if !classes.is_empty() {
            let file = File::create(&format!("{:?}-{}.cbor", campus, term))?;
            serde_cbor::to_writer(file, &classes)?;
        }
    }

    Ok(())
}
