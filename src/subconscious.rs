use crate::task_manager::{Task, TaskManager, TaskStatus};
use crate::llm_client::LLMClient;
use std::collections::VecDeque;
use log::info;

#[derive(Debug)] // Adding Debug trait here
pub struct Subconscious {
    pub task_manager: TaskManager,
    pub llm_client: LLMClient,
    pub short_term_memory: VecDeque<String>,
    pub long_term_memory: Vec<String>,
}

impl Subconscious {
    pub fn new(task_manager: TaskManager, llm_client: LLMClient) -> Self {
        Subconscious {
            task_manager,
            llm_client,
            short_term_memory: VecDeque::with_capacity(10),
            long_term_memory: Vec::new(),
        }
    }

    pub fn add_to_short_term_memory(&mut self, entry: String) {
        if self.short_term_memory.len() == self.short_term_memory.capacity() {
            if let Some(removed) = self.short_term_memory.pop_front() {
                self.add_to_long_term_memory(removed);
            }
        }
        self.short_term_memory.push_back(entry);
    }

    pub fn add_to_long_term_memory(&mut self, entry: String) {
        self.long_term_memory.push(entry);
    }

    pub async fn process_tasks(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.task_manager.execute_tasks(&self.llm_client).await?;
        let tasks = self.task_manager.get_tasks().await;
        for task in tasks {
            if task.status == TaskStatus::Completed {
                self.learn_from_task(&task);
            }
        }
        Ok(())
    }

    pub async fn add_routine_task(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let task = Task {
            description: "Routine check".to_string(),
            action: "Perform routine check".to_string(),
            status: TaskStatus::Pending,
            is_permanent: false,
        };
        self.task_manager.add_task(task).await?;
        Ok(())
    }

    pub fn learn_from_task(&mut self, task: &Task) {
        self.add_to_short_term_memory(format!("Learned from task: {}", task.description));
    }

    pub fn recall_short_term_memory(&self) -> Vec<String> {
        self.short_term_memory.iter().cloned().collect()
    }

    pub fn recall_long_term_memory(&self) -> Vec<String> {
        self.long_term_memory.clone()
    }

    pub fn print_memories(&self) {
        info!("Short-term memory: {:?}", self.recall_short_term_memory());
        info!("Long-term memory: {:?}", self.recall_long_term_memory());
    }
}
