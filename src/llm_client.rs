use colored::*;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::error::Error;
use crate::task_manager::Task; // Import Task

#[derive(Debug, Serialize)]
struct LLMInput {
    model: String,
    prompt: String,
    stream: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct LLMOutput {
    response: String,
}

#[derive(Debug, Serialize)]
struct ModelInfoRequest {
    name: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct ModelInfoResponse {
    modelfile: String,
    parameters: String,
    template: String,
    details: ModelDetails,
}

#[derive(Debug, Serialize, Deserialize)]
struct ModelDetails {
    format: String,
    family: String,
    families: Vec<String>,
    parameter_size: String,
    quantization_level: String,
}

#[derive(Clone, Debug)]
pub struct LLMClient {
    url: String,
    client: Client,
    model: String,
}

impl LLMClient {
    pub fn new(url: &str, model: &str) -> Self {
        LLMClient {
            url: url.to_string(),
            client: Client::new(),
            model: model.to_string(),
        }
    }

    pub async fn check_llm_connection(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let show_url = self.url.replace("generate", "show");
        println!("Checking LLM connection to: {}", &show_url);

        let payload = ModelInfoRequest {
            name: self.model.clone(),
        };

        let response = self.client.post(&show_url)
            .json(&payload)
            .send()
            .await?;

        if response.status().is_success() {
            let model_info = response.json::<ModelInfoResponse>().await?;
            println!("Model Information:");
            println!("Name: {}", self.model.yellow());
            println!("Format: {}", model_info.details.format);
            println!("Parameter Size: {}", model_info.details.parameter_size);
            println!("Quantization Level: {}", model_info.details.quantization_level);
            Ok(())
        } else {
            Err(format!("LLM endpoint returned status: {} - {}", response.status(), response.text().await?).into())
        }
    }

    pub fn change_model(&mut self, model: &str) {
        self.model = model.to_string();
        println!("Changing model to {}", model);
    }

    pub async fn process_task(&self, task: &Task) -> Result<String, Box<dyn Error + Send + Sync>> {
        let input = LLMInput {
            model: self.model.clone(),
            prompt: task.description.clone(),
            stream: false,
        };

        let response = self.client.post(&self.url)
            .json(&input)
            .send()
            .await?;

        if response.status().is_success() {
            let output = response.json::<LLMOutput>().await?;
            Ok(output.response)
        } else {
            Err(format!("LLM processing failed: {} - {}", response.status(), response.text().await?).into())
        }
    }

    pub async fn process_query(&self, query: &str, tasks: Vec<Task>) -> Result<String, Box<dyn Error + Send + Sync>> {
        let task_descriptions: Vec<String> = tasks.into_iter().map(|task| task.description).collect();
        let task_info = format!("Current tasks: {:?}", task_descriptions);

        let input = LLMInput {
            model: self.model.clone(),
            prompt: format!("{}\n{}", task_info, query),
            stream: false,
        };

        let response = self.client.post(&self.url)
            .json(&input)
            .send()
            .await?;

        if response.status().is_success() {
            let output = response.json::<LLMOutput>().await?;
            Ok(output.response)
        } else {
            Err(format!("LLM processing failed: {} - {}", response.status(), response.text().await?).into())
        }
    }
}
