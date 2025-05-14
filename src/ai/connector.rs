// src/ai/connector.rs
use anyhow::Result;

/// Trait defining the interface for AI processing
pub trait AiConnector: Send + Sync {
    /// Process an image and return the AI's response
    fn process_image(&mut self, image_data: &[u8]) -> Result<String>;
}