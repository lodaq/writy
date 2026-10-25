//! Exercise the single-request, offline writing bundle with synthetic prose.
use anyhow::Result;
use std::time::Instant;
use writy_core::{
    config::Config,
    provider::{ModelService, WritingAssistance},
    types::{Capability, Confidence, Language, TextSnapshot},
};
#[tokio::main]
async fn main() -> Result<()> {
    let service = ModelService::new(Config {
        offline: true,
        ..Config::default()
    })?;
    let snapshot = TextSnapshot {
        source: "synthetic-writing-bundle".into(),
        focus_epoch: 1,
        revision: 1,
        language: Language::English,
        text: "I need to make a decision.".into(),
        confidence: Confidence::Projected,
    };
    let start = Instant::now();
    let result = service
        .generate(snapshot, Capability::WritingAssistance)
        .await?;
    // This example uses fixed synthetic input, so its response is safe to inspect.
    println!("Synthetic bundle: {}", result.text);
    let parsed = WritingAssistance::parse(&result.text)?;
    println!(
        "{}",
        serde_json::json!({"elapsed_ms":start.elapsed().as_millis(),"provider":result.provider,"output":serde_json::from_str::<serde_json::Value>(&result.text)?,"lexical_categories":[parsed.definitions.len(),parsed.examples.len(),parsed.collocations.len(),parsed.synonyms.len(),parsed.antonyms.len(),parsed.word_family.len()]})
    );
    Ok(())
}
