use tokio::main;
use crate::task_manager::{Task, TaskManager, TaskStatus};
use crate::core_loop::core_loop;
use crate::subconscious::Subconscious;
use crate::llm_client::LLMClient;
use config::Config;
use std::sync::Arc;
use std::thread;
use warp::Filter;
use tokio::sync::Mutex;
use log::{info, debug, error};
use std::fs::OpenOptions;
use log::LevelFilter;
use env_logger::{Builder, Target};
use warp::reject::Reject;
use serde::Deserialize;

mod task_manager;
mod core_loop;
mod subconscious;
mod llm_client;

#[derive(Debug)]
struct CustomError;

impl Reject for CustomError {}

#[derive(Deserialize)]
struct Query {
    query: String,
}

#[main]
async fn main() {
    // Set up logging to a file
    let file = OpenOptions::new()
        .append(true)
        .create(true)
        .open("subconscious_ai.log")
        .unwrap();
    Builder::new()
        .target(Target::Pipe(Box::new(file)))
        .filter_level(LevelFilter::Debug)
        .init();

    info!("Starting application...");

    // Load settings from config file
    let settings = Config::builder()
        .add_source(config::File::with_name("config"))
        .build()
        .unwrap();

    let redis_url = settings.get_string("redis.url").unwrap();
    let llm_url = settings.get_string("llm.url").unwrap();
    let model_name = settings.get_string("llm.model").unwrap();
    let llm_client = LLMClient::new(&llm_url, &model_name);
    let task_manager = TaskManager::new(&redis_url);

    let subconscious = Arc::new(Mutex::new(Subconscious::new(task_manager.clone(), llm_client.clone())));

    // Add the persistent task at startup
    let persistent_tasks = vec![
        Task {
            description: "Self Health Check".to_string(),
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
    ];

    for task in persistent_tasks {
        if let Err(e) = task_manager.add_task(task.clone()).await {
            error!("Failed to add persistent task: {:?}", e);
        } else {
            info!("Added persistent task: {:?}", task);
        }
    }

    // Shared state for API server
    let state = Arc::new(Mutex::new(SomeSharedState::new(task_manager.clone(), llm_client.clone(), subconscious.clone())));

    // Clone the state for API thread
    let api_state = state.clone();

    // Spawn a thread for the API server
    let api_thread = thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let state_filter = warp::any().map(move || api_state.clone());

            // Define API routes
            let hello_route = warp::path!("hello").map(|| "Hello from the API!");
            let get_tasks = warp::path("tasks")
                .and(warp::get())
                .and(state_filter.clone())
                .and_then(|state: Arc<Mutex<SomeSharedState>>| async move {
                    debug!("Received request to get tasks");
                    let state = state.lock().await;
                    let tasks = state.task_manager.get_tasks().await;
                    debug!("Returning tasks: {:?}", tasks);
                    Ok::<_, warp::Rejection>(warp::reply::json(&tasks))
                });

            let add_task = warp::path("add_task")
                .and(warp::post())
                .and(warp::body::json())
                .and(state_filter.clone())
                .and_then(|task: Task, state: Arc<Mutex<SomeSharedState>>| async move {
                    debug!("Received request to add task: {:?}", task);
                    {
                        let state = state.lock().await;
                        debug!("Adding task to task manager: {:?}", task);
                        if let Err(e) = state.task_manager.add_task(task.clone()).await {
                            error!("Failed to add task via API: {:?}", e);
                            return Err(warp::reject::custom(CustomError));
                        }
                        debug!("Task added to task manager: {:?}", task);
                    }
                    info!("Task added via API: {:?}", task);
                    Ok::<_, warp::Rejection>(warp::reply::with_status("Task added", warp::http::StatusCode::OK))
                });

            let validate_task = warp::path("validate_task")
                .and(warp::post())
                .and(warp::body::json())
                .and(state_filter.clone())
                .and_then(|task: Task, state: Arc<Mutex<SomeSharedState>>| async move {
                    debug!("Received request to validate task: {:?}", task);
                    {
                        let state = state.lock().await;
                        debug!("Validating task: {:?}", task);
                        if let Err(e) = state.task_manager.update_task_status(&task, TaskStatus::Completed).await {
                            error!("Failed to validate task via API: {:?}", e);
                            return Err(warp::reject::custom(CustomError));
                        }
                        debug!("Task validated: {:?}", task);
                    }
                    info!("Task validated via API: {:?}", task);
                    Ok::<_, warp::Rejection>(warp::reply::with_status("Task validated", warp::http::StatusCode::OK))
                });

            let change_model = warp::path!("change_model" / String)
                .and(warp::post())
                .and(state_filter.clone())
                .and_then(|model: String, state: Arc<Mutex<SomeSharedState>>| async move {
                    let mut state = state.lock().await;
                    debug!("Changing model to: {}", model);
                    state.llm_client.change_model(&model);
                    Ok::<_, warp::Rejection>(warp::reply::json(&format!("Model changed to: {}", model)))
                });

            let ask_llm = warp::path("ask_llm")
                .and(warp::post())
                .and(warp::body::json())
                .and(state_filter.clone())
                .and_then(|query: Query, state: Arc<Mutex<SomeSharedState>>| async move {
                    debug!("Received query: {}", query.query);
                    let state = state.lock().await;
                    let tasks = state.task_manager.get_tasks().await;
                    match state.llm_client.process_query(&query.query, tasks).await {
                        Ok(response) => {
                            info!("LLM response: {}", response);
                            Ok::<_, warp::Rejection>(warp::reply::json(&response))
                        }
                        Err(e) => {
                            error!("Failed to process query with LLM: {:?}", e);
                            Err(warp::reject::custom(CustomError))
                        }
                    }
                });

            let status_route = warp::path("status")
                .and(warp::get())
                .and(state_filter)
                .and_then(|state: Arc<Mutex<SomeSharedState>>| async move {
                    let state = state.lock().await;
                    let status = state.get_status();
                    debug!("Returning status: {:?}", status);
                    Ok::<_, warp::Rejection>(warp::reply::json(&status))
                });

            let routes = hello_route.or(get_tasks).or(add_task).or(validate_task).or(change_model).or(ask_llm).or(status_route);

            // Combine routes and serve
            warp::serve(routes)
                .run(([0, 0, 0, 0], 3030))
                .await;
        });
    });

    // Start the core loop
    tokio::spawn(async move {
        core_loop(subconscious).await;
    });

    // Wait for the API thread to finish (if needed)
    api_thread.join().unwrap();
}
// Example shared state struct
#[derive(Debug)]
struct SomeSharedState {
    task_manager: TaskManager,
    llm_client: LLMClient,
    #[allow(dead_code)] // Add this line to suppress the warning
    subconscious: Arc<Mutex<Subconscious>>,
}


impl SomeSharedState {
    fn new(task_manager: TaskManager, llm_client: LLMClient, subconscious: Arc<Mutex<Subconscious>>) -> Self {
        SomeSharedState {
            task_manager,
            llm_client,
            subconscious,
        }
    }

    fn get_status(&self) -> String {
        // Return detailed status of the program
        format!("Tasks: {:?}, LLM Client: {:?}", self.task_manager, self.llm_client)
    }
}
