//! URL: `https://dblp.org/search/publ/api?format=json&q={query}`

use crate::Result;

pub fn search(query: &str) -> Result<response::Response> {
    reqwest::blocking::Client::new()
        .get("https://dblp.org/search/publ/api")
        .query(&[("format", "json"), ("q", query)])
        .send()?
        .json()
        .map_err(Into::into)
}

impl response::Hit {
    /// Download .bib
    ///
    /// Stored at `https://dblp.org/rec/{key}.bib?param=1`
    pub fn bib(&self) -> Result<String> {
        reqwest::blocking::Client::new()
            .get(format!(
                "https://dblp.org/rec/{}.bib?param=1",
                self.info.key
            ))
            .send()?
            .text()
            .map_err(Into::into)
    }
}

pub mod response {
    // Example code that deserializes and serializes the model.
    // extern crate serde;
    // #[macro_use]
    // extern crate serde_derive;
    // extern crate serde_json;
    //
    // use generated_module::Response;
    //
    // fn main() {
    //     let json = r#"{"answer": 42}"#;
    //     let model: Response = serde_json::from_str(&json).unwrap();
    // }

    use serde::{Deserialize, Serialize};

    #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
    pub struct Response {
        pub result: Result,
    }

    #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
    pub struct Result {
        pub query: String,
        pub status: Status,
        pub time: Time,
        pub completions: Completions,
        pub hits: Hits,
    }

    #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
    pub struct Completions {
        #[serde(rename = "@total")]
        pub total: String,
        #[serde(rename = "@computed")]
        pub computed: String,
        #[serde(rename = "@sent")]
        pub sent: String,
        pub c: C,
    }

    #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
    pub struct C {
        #[serde(rename = "@sc")]
        pub sc: String,
        #[serde(rename = "@dc")]
        pub dc: String,
        #[serde(rename = "@oc")]
        pub oc: String,
        #[serde(rename = "@id")]
        pub id: String,
        pub text: String,
    }

    #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
    pub struct Hits {
        #[serde(rename = "@total")]
        pub total: String,
        #[serde(rename = "@computed")]
        pub computed: String,
        #[serde(rename = "@sent")]
        pub sent: String,
        #[serde(rename = "@first")]
        pub first: String,
        pub hit: Vec<Hit>,
    }

    #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
    pub struct Hit {
        #[serde(rename = "@score")]
        pub score: String,
        #[serde(rename = "@id")]
        pub id: String,
        pub info: Info,
        pub url: String,
    }

    #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
    pub struct Info {
        pub authors: Authors,
        pub title: String,
        pub venue: String,
        pub pages: Option<String>,
        pub year: String,
        #[serde(rename = "type")]
        pub info_type: String,
        pub access: String,
        pub key: String,
        pub doi: Option<String>,
        pub ee: String,
        pub url: String,
        pub volume: Option<String>,
        pub number: Option<String>,
    }

    #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
    pub struct Authors {
        pub author: Vec<Author>,
    }

    #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
    pub struct Author {
        #[serde(rename = "@pid")]
        pub pid: String,
        pub text: String,
    }

    #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
    pub struct Status {
        #[serde(rename = "@code")]
        pub code: String,
        pub text: String,
    }

    #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
    pub struct Time {
        #[serde(rename = "@unit")]
        pub unit: String,
        pub text: String,
    }
}
