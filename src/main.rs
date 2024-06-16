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

// Import modules from their respective paths
mod core {
    pub mod core_loop;
    pub mod llm_client;
    pub mod subconscious;
    pub mod task_manager;
}
mod lln {
    pub mod neural_network;
}
mod plugins {
    pub mod plugin_manager;
}
mod tasks; // Import the tasks module

use core::task_manager::{TaskManager, Task, TaskStatus};
use core::core_loop::core_loop;
use core::subconscious::Subconscious;
use core::llm_client::LLMClient;
use plugins::plugin_manager::PluginManager;

#[derive(Debug)]
struct CustomError;

impl Reject for CustomError {}

#[derive(Deserialize)]
struct Query {
    query: String,
}

// Define the Plugin trait
pub trait Plugin {
    fn initialize(&self);
    fn execute(&self);
}

#[tokio::main]
async fn main() {
    setup_logging();
    info!("Starting application...");

    let settings = load_settings();
    let (redis_url, llm_url, model_name) = extract_settings(&settings);

    let llm_client = LLMClient::new(&llm_url, &model_name);
    let task_manager = TaskManager::new(&redis_url);
    let subconscious = Arc::new(Mutex::new(Subconscious::new(task_manager.clone(), llm_client.clone())));

    add_persistent_tasks(&task_manager).await;

    let state = Arc::new(Mutex::new(SomeSharedState::new(task_manager.clone(), llm_client.clone(), subconscious.clone())));

    let api_thread = spawn_api_server(state.clone());

    initialize_plugins();

    start_core_loop(subconscious);

    api_thread.join().unwrap();
}

fn setup_logging() {
    let file = OpenOptions::new()
        .append(true)
        .create(true)
        .open("subconscious_ai.log")
        .unwrap();
    Builder::new()
        .target(Target::Pipe(Box::new(file)))
        .filter_level(LevelFilter::Debug)
        .init();
}

fn load_settings() -> Config {
    Config::builder()
        .add_source(config::File::with_name("config"))
        .build()
        .unwrap()
}

fn extract_settings(settings: &Config) -> (String, String, String) {
    let redis_url = settings.get_string("redis.url").unwrap();
    let llm_url = settings.get_string("llm.url").unwrap();
    let model_name = settings.get_string("llm.model").unwrap();
    (redis_url, llm_url, model_name)
}

async fn add_persistent_tasks(task_manager: &TaskManager) {
    let persistent_tasks = tasks::get_persistent_tasks(); // Fetch tasks from tasks.rs
    for task in persistent_tasks {
        if let Err(e) = task_manager.add_task(task.clone()).await {
            error!("Failed to add persistent task: {:?}", e);
        } else {
            info!("Added persistent task: {:?}", task);
        }
    }
}

fn spawn_api_server(state: Arc<Mutex<SomeSharedState>>) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let state_filter = warp::any().map(move || state.clone());

            // Define API routes
            let hello_route = warp::path!("hello").map(|| "Hello from the API!");
            let get_tasks = warp::path("tasks")
                .and(warp::get())
                .and(state_filter.clone())
                .and_then(handle_get_tasks);
            let add_task = warp::path("add_task")
                .and(warp::post())
                .and(warp::body::json())
                .and(state_filter.clone())
                .and_then(handle_add_task);
            let validate_task = warp::path("validate_task")
                .and(warp::post())
                .and(warp::body::json())
                .and(state_filter.clone())
                .and_then(handle_validate_task);
            let change_model = warp::path!("change_model" / String)
                .and(warp::post())
                .and(state_filter.clone())
                .and_then(handle_change_model);
            let ask_llm = warp::path("ask_llm")
                .and(warp::post())
                .and(warp::body::json())
                .and(state_filter.clone())
                .and_then(handle_ask_llm);
            let status_route = warp::path("status")
                .and(warp::get())
                .and(state_filter)
                .and_then(handle_status);

            let routes = hello_route.or(get_tasks).or(add_task).or(validate_task).or(change_model).or(ask_llm).or(status_route);

            // Combine routes and serve
            warp::serve(routes)
                .run(([0, 0, 0, 0], 3030))
                .await;
        });
    })
}

async fn handle_get_tasks(state: Arc<Mutex<SomeSharedState>>) -> Result<impl warp::Reply, warp::Rejection> {
    debug!("Received request to get tasks");
    let state = state.lock().await;
    let tasks = state.task_manager.get_tasks().await;
    debug!("Returning tasks: {:?}", tasks);
    Ok(warp::reply::json(&tasks))
}

async fn handle_add_task(task: Task, state: Arc<Mutex<SomeSharedState>>) -> Result<impl warp::Reply, warp::Rejection> {
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
    Ok(warp::reply::with_status("Task added", warp::http::StatusCode::OK))
}

async fn handle_validate_task(task: Task, state: Arc<Mutex<SomeSharedState>>) -> Result<impl warp::Reply, warp::Rejection> {
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
    Ok(warp::reply::with_status("Task validated", warp::http::StatusCode::OK))
}

async fn handle_change_model(model: String, state: Arc<Mutex<SomeSharedState>>) -> Result<impl warp::Reply, warp::Rejection> {
    let mut state = state.lock().await;
    debug!("Changing model to: {}", model);
    state.llm_client.change_model(&model);
    Ok(warp::reply::json(&format!("Model changed to: {}", model)))
}

async fn handle_ask_llm(query: Query, state: Arc<Mutex<SomeSharedState>>) -> Result<impl warp::Reply, warp::Rejection> {
    debug!("Received query: {}", query.query);
    let state = state.lock().await;
    let tasks = state.task_manager.get_tasks().await;
    match state.llm_client.process_query(&query.query, tasks).await {
        Ok(response) => {
            info!("LLM response: {}", response);
            Ok(warp::reply::json(&response))
        }
        Err(e) => {
            error!("Failed to process query with LLM: {:?}", e);
            Err(warp::reject::custom(CustomError))
        }
    }
}

async fn handle_status(state: Arc<Mutex<SomeSharedState>>) -> Result<impl warp::Reply, warp::Rejection> {
    let state = state.lock().await;
    let status = state.get_status();
    debug!("Returning status: {:?}", status);
    Ok(warp::reply::json(&status))
}

fn initialize_plugins() {
    let plugin_manager = PluginManager::new();
    plugin_manager.initialize_plugins();
    plugin_manager.execute_plugins();
}

fn start_core_loop(subconscious: Arc<Mutex<Subconscious>>) {
    tokio::spawn(async move {
        core_loop(subconscious).await;
    });
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
