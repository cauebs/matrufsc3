use std::collections::HashMap;

use anyhow::Result;
use futures::{stream::FuturesUnordered, StreamExt, TryFutureExt};
use reqwest::Client;
use select::{
    document::Document,
    predicate::{Attr, Name, Predicate},
};

mod parse;
use parse::Class;

const CAGR_URL: &str = "https://cagr.sistemas.ufsc.br/modules/comunidade/cadastroTurmas/";

#[derive(Clone, Copy, Debug)]
pub enum Campus {
    EAD = 0,
    FLO = 1,
    JOI = 2,
    CBS = 3,
    ARA = 4,
    BLN = 5,
}

impl std::fmt::Display for Campus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::EAD => "EAD",
                Self::FLO => "FLO",
                Self::JOI => "JOI",
                Self::CBS => "CBS",
                Self::ARA => "ARA",
                Self::BLN => "BLN",
            }
        )
    }
}

fn form_data(campus: Campus, term: String, page_index: usize) -> HashMap<&'static str, String> {
    let mut form = HashMap::new();
    form.insert("formBusca", "formBusca".to_owned());
    form.insert("javax.faces.ViewState", "j_id1".to_owned());
    form.insert("formBusca:selectSemestre", term);
    form.insert("formBusca:selectCampus", (campus as u32).to_string());
    form.insert("formBusca:dataScroller1", page_index.to_string());
    form
}

pub struct Cagr {
    client: Client,
    campus: Campus,
    term: String,
}

// why use a struct? because the http client needs to be in a specific state
// this way we ensure via the type system that no steps can be skipped
// or done in the wrong order by mistake

impl Cagr {
    pub async fn new(campus: Campus, term: String) -> Result<Self> {
        let client = Client::builder().cookie_store(true).build()?;

        // the site requires the client to be primed with the cookies
        client.post(CAGR_URL).send().await?;

        Ok(Self {
            client,
            campus,
            term,
        })
    }

    pub async fn page_count(&self) -> Result<usize> {
        let response = self
            .client
            .post(CAGR_URL)
            .form(&form_data(self.campus, self.term.clone(), 2)) // page 1 does not work
            .send()
            .await?;

        let contents = response.text().await?;
        let document = Document::from(contents.as_ref());

        let entries = document
            .find(
                Name("span")
                    .and(Attr("id", "formBusca:dataTableGroup"))
                    .child(Name("span")),
            )
            .next()
            .and_then(|node| node.inner_html().parse::<u32>().ok())
            .unwrap_or(0); // TODO: error out on this

        let pages = (f64::from(entries) / 50.).ceil() as usize;

        Ok(pages)
    }

    pub async fn scrape(self) -> Result<(Campus, String, Vec<Class>)> {
        let mut entries = Vec::new();

        let mut previous: Option<String> = None;
        for page_index in 1..=self.page_count().await? {
            let response = self
                .client
                .post(CAGR_URL)
                .form(&form_data(self.campus, self.term.clone(), page_index))
                .send()
                .await?;

            // TODO: check response url

            let contents = response.text().await?;

            if Some(&contents) == previous.as_ref() {
                break;
            }

            println!("{} {:?} {}", self.term, self.campus, page_index);
            entries.extend(parse::classes_from_html(&contents)?);
            // TODO: handle/ignore errors where appropriate, instead of bubbling them up
            previous.replace(contents.clone());
        }

        Ok((self.campus, self.term, entries))
    }
}

pub async fn available_terms() -> Result<Vec<String>> {
    let response = reqwest::get(CAGR_URL).await?;
    let contents = response.text().await?;
    let document = Document::from(contents.as_ref());

    let dropdown = Name("select").and(Attr("id", "formBusca:selectSemestre"));
    let options = dropdown.child(Name("option"));

    Ok(document
        .find(options)
        .filter_map(|option| option.attr("value"))
        .map(|value| value.to_owned())
        .collect())
}

pub async fn scrape_last_n_terms(
    n: usize,
    campi: &[Campus],
) -> Vec<Result<(Campus, String, Vec<Class>)>> {
    let terms = available_terms()
        .await
        .unwrap()
        .into_iter()
        .take(n)
        .collect::<Vec<_>>();

    let tasks = FuturesUnordered::new();

    for term in terms {
        for &campus in campi {
            tasks.push(Cagr::new(campus, term.clone()).and_then(Cagr::scrape));
        }
    }

    tasks.collect().await
}
