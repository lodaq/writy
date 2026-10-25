//! Synthetic, offline-only model checks. No credentials or user prose are read.
use anyhow::Result;
use std::time::Instant;
use writy_core::{
    config::Config,
    provider::ModelService,
    types::{Capability, Confidence, Language, TextSnapshot},
};

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config {
        offline: true,
        ..Config::default()
    };
    let service = ModelService::new(config)?;
    let samples = [
        (
            Capability::Translate,
            Language::English,
            "I will attend the meeting tomorrow.",
        ),
        (
            Capability::Translate,
            Language::Arabic,
            "سأحضر الاجتماع غدًا.",
        ),
        (
            Capability::Rewrite,
            Language::English,
            "I am write to ask about the meeting tomorrow.",
        ),
        (
            Capability::Rewrite,
            Language::Arabic,
            "انا اكتب لكي اسئل عن اجتماع الغد.",
        ),
    ];
    for (index, (capability, language, text)) in samples.into_iter().enumerate() {
        let snapshot = TextSnapshot {
            source: "synthetic-model-smoke".into(),
            focus_epoch: 1,
            revision: index as u64,
            language,
            text: text.into(),
            confidence: Confidence::Verified,
        };
        let start = Instant::now();
        match service.generate(snapshot, capability).await {
            Ok(result) => println!(
                "{}",
                serde_json::json!({"capability":capability,"language":language,"provider":result.provider,"elapsed_ms":start.elapsed().as_millis(),"output":result.text})
            ),
            Err(error) => return Err(error),
        }
    }
    Ok(())
}
