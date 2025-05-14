// src/ai/local_model.rs
use anyhow::{Result, anyhow};
use log::{info, warn};
use serde::{Serialize, Deserialize};
use reqwest::blocking::Client;
use base64::{Engine as _, engine::general_purpose};
use std::time::Duration;

use super::connector::AiConnector;

//Implementation for Ollama local LLM processing
pub struct LocalModel {
    ollama_url: String,
    model_name: String,
    client: Client,
    prompt: String,
}

#[derive(Serialize)]
struct OllamaRequest {
    model: String,
    prompt: String,
    images: Option<Vec<String>>,
    stream: bool,
}

#[derive(Deserialize)]
struct OllamaResponse {
    response: String,
}

impl LocalModel {
    pub fn new(model_path: &str) -> Result<Self> {
        //For Ollama, model_path is actually the model name (e.g., "llava:latest")
        //default Ollama URL is localhost:11434
        let ollama_url = std::env::var("OLLAMA_HOST").unwrap_or_else(|_| "http://localhost:11434".to_string());
        
        info!("Initializing Ollama model: {} at {}", model_path, ollama_url);
        
        let client = Client::builder()
            .timeout(Duration::from_secs(300)) // 5 minutes
            .connect_timeout(Duration::from_secs(10))
            .build()?;
        
        //check if Ollama is running
        let check_url = format!("{}/api/tags", ollama_url);
        match client.get(&check_url).send() {
            Ok(response) => {
                if !response.status().is_success() {
                    warn!("Ollama server responded with status: {}", response.status());
                }
            }
            Err(e) => {
                warn!("Could not connect to Ollama server at {}: {}", ollama_url, e);
                warn!("Make sure Ollama is running: 'ollama serve'");
            }
        }
        
        let default_prompt = "Describe what you see in this image in detail, focusing on any text, UI elements, and visual content.".to_string();
        
        Ok(Self {
            ollama_url,
            model_name: model_path.to_string(),
            client,
            prompt: default_prompt,
        })
    }
    
    //Set a custom prompt for image analysis
    pub fn set_prompt(&mut self, prompt: &str) {
        self.prompt = prompt.to_string();
    }
    
    //Reset to the default prompt
    pub fn reset_prompt(&mut self) {
        self.prompt = "Describe what you see in this image in detail, focusing on any text, UI elements, and visual content.".to_string();
    }
    
    //Check if the specified model is available
    fn check_model_available(&self) -> Result<bool> {
        let url = format!("{}/api/tags", self.ollama_url);
        let response = self.client.get(&url).send()?;
        
        if !response.status().is_success() {
            return Ok(false);
        }
        
        let tags: serde_json::Value = response.json()?;
        
        //Check if our model is in the list
        if let Some(models) = tags["models"].as_array() {
            for model in models {
                if let Some(name) = model["name"].as_str() {
                    if name == self.model_name {
                        return Ok(true);
                    }
                }
            }
        }
        
        Ok(false)
    }
}

impl AiConnector for LocalModel {
    fn process_image(&mut self, image_data: &[u8]) -> Result<String> {
        //Check if Ollama is running and model is available
        if !self.check_model_available()? {
            return Err(anyhow!("Model '{}' not found. Pull it with: ollama pull {}", self.model_name, self.model_name));
        }
        
        info!("Processing image with Ollama model: {}", self.model_name);
        info!("This may take a while on first run as the model loads into memory...");
        
        // Convert image to base64
        let base64_image = general_purpose::STANDARD.encode(image_data);
        
        //Construct the request
        let request = OllamaRequest {
            model: self.model_name.clone(),
            prompt: self.prompt.clone(),
            images: Some(vec![base64_image]),
            stream: false,
        };
        
        //send the request to Ollama
        let url = format!("{}/api/generate", self.ollama_url);
        
        info!("Sending request to Ollama... (this may take up to 5 minutes)");
        
        let response = self.client
            .post(&url)
            .json(&request)
            .send()
            .map_err(|e| {
                if e.is_timeout() {
                    anyhow!("Request timed out after 5 minutes. The model might be too large or your system may need more resources.")
                } else {
                    anyhow!("Ollama API error: {}", e)
                }
            })?;
        
        if !response.status().is_success() {
            let error_text = response.text()?;
            return Err(anyhow!("Ollama API error: {}", error_text));
        }
        
        //parse the response
        let response_data: OllamaResponse = response.json()?;
        
        Ok(response_data.response)
    }
}