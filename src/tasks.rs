use crate::core::task_manager::{Task, TaskStatus};

pub fn get_persistent_tasks() -> Vec<Task> {
    vec![
        Task {
            description: "Self Health Check: check then task assigned, make a bulletpoint list and start working on it".to_string(),
            action: "check_status".to_string(),
            status: TaskStatus::Pending,
            is_permanent: true,
        },
        Task {
            description: "Read from Redis".to_string(),
            action: "display_redis_data".to_string(),
            status: TaskStatus::Pending,
            is_permanent: true,
        },
        Task {
            description: "Start LLM interaction".to_string(),
            action: "start_llm_communications".to_string(),
            status: TaskStatus::Pending,
            is_permanent: true,
        },
        Task {
            description: "Log Analysis".to_string(),
            action: "comment_last_logs".to_string(),
            status: TaskStatus::Pending,
            is_permanent: true,
        },
        Task {
            description: "Self analysis and new tasks".to_string(),
            action: "take_improvement_actions".to_string(),
            status: TaskStatus::Pending,
            is_permanent: true,
        },
        Task {
            description: "Write a detailed report of concepts and behaviors learned so far".to_string(),
            action: "write_detailed_report".to_string(),
            status: TaskStatus::Pending,
            is_permanent: true,
        }
    ]
}
