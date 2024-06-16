use redis::AsyncCommands;
use redis::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;
use log::{info, error, debug};
use crate::llm_client::LLMClient;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Task {
    pub description: String,
    pub action: String,
    pub status: TaskStatus,
    pub is_permanent: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum TaskStatus {
    Pending,
    InProgress,
    Completed,
}

#[derive(Clone, Debug)]
pub struct TaskManager {
    redis_client: Arc<Mutex<Client>>,
}

impl TaskManager {
    pub fn new(redis_url: &str) -> Self {
        let client = Client::open(redis_url).expect("Invalid Redis URL");
        TaskManager {
            redis_client: Arc::new(Mutex::new(client)),
        }
    }

    pub async fn add_task(&self, task: Task) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let serialized_task = serde_json::to_string(&task)?;
        let mut con = self.redis_client.lock().await.get_multiplexed_async_connection().await?;
        con.lpush("tasks", serialized_task.clone()).await?;
        if task.is_permanent {
            con.lpush("persistent_tasks", serialized_task).await?;
        }
        Ok(())
    }

    pub async fn update_task_status(&self, task: &Task, new_status: TaskStatus) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut con = self.redis_client.lock().await.get_multiplexed_async_connection().await?;
        let tasks_json: Vec<String> = con.lrange("tasks", 0, -1).await?;
        
        let mut updated_tasks = Vec::new();
        for task_json in tasks_json {
            let mut task_in_list: Task = serde_json::from_str(&task_json)?;
            if task_in_list.description == task.description && task_in_list.action == task.action {
                task_in_list.status = new_status.clone();
            }
            updated_tasks.push(serde_json::to_string(&task_in_list)?);
        }
        
        con.del("tasks").await?;
        for updated_task in updated_tasks {
            con.rpush("tasks", updated_task).await?;
        }
        Ok(())
    }

    pub async fn execute_tasks(&self, llm_client: &LLMClient) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut con = self.redis_client.lock().await.get_multiplexed_async_connection().await.unwrap();
        while let Some(task_json) = con.lpop::<_, Option<String>>("tasks", None).await.unwrap() {
            let task: Task = serde_json::from_str(&task_json).unwrap();
            if task.status == TaskStatus::Pending {
                debug!("Executing task: {:?}", task);

                match llm_client.process_task(&task).await {
                    Ok(result) => {
                        info!("Task processed with result: {}", result);
                        // Store result in Redis
                        let _: () = con.set(&task.description, result).await.unwrap();
                        match self.update_task_status(&task, TaskStatus::Completed).await {
                            Ok(_) => info!("Task completed and status updated: {:?}", task),
                            Err(e) => error!("Failed to update task status: {:?}", e),
                        }
                    }
                    Err(e) => error!("Failed to process task with LLM: {:?}", e),
                }
            }
        }
        Ok(())
    }

    pub async fn check_redis_connection(&self) -> Result<(), redis::RedisError> {
        let mut con = self.redis_client.lock().await.get_multiplexed_async_connection().await?;
        let _: () = con.set_ex("redis_connection_check", "OK", 10).await?;
        Ok(())
    }

    pub async fn get_tasks(&self) -> Vec<Task> {
        let mut con = self.redis_client.lock().await.get_multiplexed_async_connection().await.unwrap();
        let tasks_json: Vec<String> = con.lrange("tasks", 0, -1).await.unwrap_or_default();
        
        let mut tasks = Vec::new();
        for task_json in tasks_json {
            if let Ok(task) = serde_json::from_str::<Task>(&task_json) {
                tasks.push(task);
            }
        }
        tasks
    }

    // Commenting out unused method to suppress warning
    // pub async fn get_completed_tasks(&self) -> Vec<Task> {
    //     match self.redis_client.lock().await.get_multiplexed_async_connection().await {
    //         Ok(mut con) => {
    //             let tasks_json: Vec<String> = con.lrange("completed_tasks", 0, -1).await.unwrap();
    //             debug!("Retrieved completed tasks JSON from Redis: {:?}", tasks_json);
    //             let tasks: Vec<Task> = tasks_json.into_iter().map(|task_json| {
    //                 let task: Task = serde_json::from_str(&task_json).unwrap();
    //                 task
    //             }).collect();
    //             debug!("Deserialized completed tasks: {:?}", tasks);
    //             tasks
    //         },
    //         Err(e) => {
    //             error!("Failed to get Redis connection: {:?}", e);
    //             vec![]
    //         },
    //     }
    // }
}