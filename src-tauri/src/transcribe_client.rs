use crate::constants::API_BASE_URL;
use anyhow::{Context, Result};
use reqwest::Client;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct TranscriptionResponse {
    text: String,
    original_text: Option<String>,
}

pub struct TranscribeClient {
    http_client: Client,
}

impl TranscribeClient {
    pub fn new() -> Self {
        Self {
            http_client: Client::new(),
        }
    }

    pub async fn fetch_transcription(&self, recording: Vec<u8>) -> Result<String> {
        let res = self
            .http_client
            .post(format!("{API_BASE_URL}/transcribe"))
            .header("Content-Type", "audio/wav")
            .body(recording)
            .send()
            .await?;

        let res: TranscriptionResponse = res.json().await?;

        Ok(res.text)
    }

    pub async fn clean_transcription(&self, transcription: String) -> Result<String> {
        let res = self
            .http_client
            .post(format!("{API_BASE_URL}/clean-transcription"))
            .header("Content-Type", "application/json")
            .body(serde_json::json!({ "text": transcription }).to_string())
            .send()
            .await?;

        let response: TranscriptionResponse = res.json().await?;

        let _original_text =
            response.original_text.context("Failed to get original text")?;

        Ok(response.text)
    }
}
