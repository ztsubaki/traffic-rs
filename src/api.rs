use crate::{CONFIG, Simulator, conf, net::*, simulator::*};
use axum::{
    Json, Router,
    extract::{
        Path, Query, State,
        rejection::{PathRejection, QueryRejection},
    },
    http::{StatusCode, header},
    response::sse::Event,
    response::{IntoResponse, Response, Sse},
    routing,
};
use chrono::Local;
use futures::Stream;
use serde::{Deserialize, Serialize};
use std::{
    convert::Infallible,
    net::SocketAddr,
    process::ExitCode,
    sync::{Arc, Mutex, RwLock, mpsc::Sender},
    time::Duration,
};
use sysinfo::{MemoryRefreshKind, ProcessRefreshKind, System, get_current_pid};
use tokio_stream::wrappers::ReceiverStream;

const API_PREFIX: &str = "/api/v1";
const PROBLEM_JSON: &str = "application/problem+json";

pub async fn run_api(
    simulator: Arc<Mutex<Simulator>>,
    net: Arc<Net>,
    tx: Sender<usize>,
    rx: tokio::sync::mpsc::Receiver<Result<RunStat, String>>,
) -> ExitCode {
    let sys = Arc::new(RwLock::new(System::new_all()));
    let state = ApiState {
        gate: Arc::new(tokio::sync::Mutex::new(())),
        simulation_gate: Arc::new(tokio::sync::RwLock::new(())),
        simulator,
        net,
        compute_tx: tx,
        compute_rx: Arc::new(tokio::sync::Mutex::new(rx)),
        sys: sys.clone(),
    };
    let api = Router::new()
        .route("/health", routing::get(health))
        .route("/simulation", routing::get(simulation))
        .route("/simulation/run", routing::get(run_simulation))
        .route("/network/statistics", routing::get(network_statistics))
        .route("/traffic-lights/{id}", routing::get(traffic_light))
        .route("/edges/{id}", routing::get(edge))
        .route("/lanes/{id}", routing::get(lane))
        .route("/junctions/{id}", routing::get(junction))
        .route("/vehicles/{id}", routing::get(vehicle));
    let app = Router::new()
        .route("/openapi.json", routing::get(openapi_document))
        .nest(API_PREFIX, api)
        .fallback(not_found)
        .with_state(state);
    let listener = match tokio::net::TcpListener::bind(SocketAddr::new(
        CONFIG.listen_addr,
        CONFIG.listen_port,
    ))
    .await
    {
        Ok(listener) => listener,
        Err(error) => {
            log::error!(
                "Failed to bind the API server to {}:{}: {}",
                CONFIG.listen_addr,
                CONFIG.listen_port,
                error
            );
            return ExitCode::FAILURE;
        }
    };
    let process_id = match get_current_pid() {
        Ok(process_id) => process_id,
        Err(error) => {
            log::error!("Failed to determine the current process ID: {error}");
            return ExitCode::FAILURE;
        }
    };
    tokio::spawn(async move {
        loop {
            {
                let mut system = match sys.write() {
                    Ok(system) => system,
                    Err(error) => {
                        log::error!("System metrics lock is poisoned: {error}");
                        return;
                    }
                };
                system.refresh_cpu_usage();
                system.refresh_memory_specifics(MemoryRefreshKind::nothing().with_ram());
                system.refresh_processes_specifics(
                    sysinfo::ProcessesToUpdate::Some(&[process_id]),
                    true,
                    ProcessRefreshKind::nothing()
                        .with_cpu()
                        .with_memory()
                        .without_tasks(),
                );
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    });
    match axum::serve(listener, app).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            log::error!("API server terminated unexpectedly: {error}");
            ExitCode::FAILURE
        }
    }
}

#[derive(Clone)]
struct ApiState {
    gate: Arc<tokio::sync::Mutex<()>>,
    simulation_gate: Arc<tokio::sync::RwLock<()>>,
    simulator: Arc<Mutex<Simulator>>,
    net: Arc<Net>,
    compute_tx: Sender<usize>,
    compute_rx: Arc<tokio::sync::Mutex<tokio::sync::mpsc::Receiver<Result<RunStat, String>>>>,
    sys: Arc<RwLock<System>>,
}

#[derive(Serialize)]
struct ProblemDetails {
    #[serde(rename = "type")]
    type_url: &'static str,
    title: &'static str,
    status: u16,
    detail: String,
}
struct ApiError {
    status: StatusCode,
    type_url: &'static str,
    title: &'static str,
    detail: String,
}
impl ApiError {
    fn bad_request(detail: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            type_url: "https://httpstatuses.com/400",
            title: "Invalid request",
            detail: detail.into(),
        }
    }
    fn not_found(resource: &str, id: usize) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            type_url: "https://httpstatuses.com/404",
            title: "Resource not found",
            detail: format!("No {resource} exists with ID {id}."),
        }
    }
    fn conflict(detail: impl Into<String>) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            type_url: "https://httpstatuses.com/409",
            title: "Simulation is running",
            detail: detail.into(),
        }
    }
    fn internal(detail: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            type_url: "https://httpstatuses.com/500",
            title: "Internal server error",
            detail: detail.into(),
        }
    }
}
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = Json(ProblemDetails {
            type_url: self.type_url,
            title: self.title,
            status: self.status.as_u16(),
            detail: self.detail,
        });
        (self.status, [(header::CONTENT_TYPE, PROBLEM_JSON)], body).into_response()
    }
}
async fn not_found() -> ApiError {
    ApiError {
        status: StatusCode::NOT_FOUND,
        type_url: "https://httpstatuses.com/404",
        title: "Endpoint not found",
        detail: "The requested API endpoint does not exist.".to_owned(),
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HealthResponse {
    service: &'static str,
    is_simulation_running: bool,
    timestamp: String,
    system: SystemStatus,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SystemStatus {
    logical_cpu_count: usize,
    process_cpu_percent: f32,
    system_cpu_percent: f32,
    total_memory_bytes: u64,
    used_memory_bytes: u64,
    process_memory_bytes: u64,
}
impl SystemStatus {
    fn snapshot(sys: &System) -> Result<Self, ApiError> {
        let process_id = get_current_pid().map_err(|error| {
            ApiError::internal(format!("Failed to determine the process ID: {error}"))
        })?;
        let process = sys.process(process_id).ok_or_else(|| {
            ApiError::internal("Process metrics are not available yet. Try again shortly.")
        })?;
        Ok(Self {
            logical_cpu_count: sys.cpus().len(),
            process_cpu_percent: process.cpu_usage(),
            system_cpu_percent: sys.global_cpu_usage(),
            total_memory_bytes: sys.total_memory(),
            used_memory_bytes: sys.used_memory(),
            process_memory_bytes: process.memory(),
        })
    }
}
async fn health(State(state): State<ApiState>) -> Result<Json<HealthResponse>, ApiError> {
    let system = state
        .sys
        .read()
        .map_err(|error| ApiError::internal(format!("System metrics lock is poisoned: {error}")))?;
    Ok(Json(HealthResponse {
        service: "traffic-rs",
        is_simulation_running: state.simulation_gate.try_read().is_err(),
        timestamp: Local::now().to_rfc3339(),
        system: SystemStatus::snapshot(&system)?,
    }))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SimulationResponse {
    configuration: conf::Config,
    start_time_millis: usize,
    end_time_millis: usize,
    step_duration_millis: usize,
    current_step: usize,
    statistics: GlobalStatistics,
}
async fn simulation(State(state): State<ApiState>) -> Result<Json<SimulationResponse>, ApiError> {
    let _simulation_gate = state.simulation_gate.try_read().map_err(|_| {
        ApiError::conflict("Simulation state cannot be read while a run is in progress.")
    })?;
    let _gate = state.gate.lock().await;
    let simulator = state.simulator.lock().unwrap();
    Ok(Json(SimulationResponse {
        configuration: CONFIG.clone(),
        start_time_millis: simulator.time_begin,
        end_time_millis: simulator.time_end,
        step_duration_millis: simulator.step,
        current_step: simulator.step_current,
        statistics: simulator.global_statistics(),
    }))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct NetworkStatistics {
    junction_count: usize,
    lane_count: usize,
    edge_count: usize,
    traffic_light_count: usize,
}
async fn network_statistics(State(state): State<ApiState>) -> Json<NetworkStatistics> {
    Json(NetworkStatistics {
        junction_count: state.net.get_num_junctions(),
        lane_count: state.net.get_num_lanes(),
        edge_count: state.net.get_num_edges(),
        traffic_light_count: state.net.get_num_trafficlights(),
    })
}

async fn resource_id(path: Result<Path<usize>, PathRejection>) -> Result<usize, ApiError> {
    path.map(|Path(id)| id).map_err(|_| {
        ApiError::bad_request("The path parameter `id` must be a non-negative integer.")
    })
}
async fn traffic_light(
    State(state): State<ApiState>,
    path: Result<Path<usize>, PathRejection>,
) -> Result<Json<TrafficLight>, ApiError> {
    let id = resource_id(path).await?;
    state
        .net
        .get_trafficlight(TrafficLightID::from_usize(id))
        .cloned()
        .map(Json)
        .ok_or_else(|| ApiError::not_found("traffic light", id))
}
async fn lane(
    State(state): State<ApiState>,
    path: Result<Path<usize>, PathRejection>,
) -> Result<Json<Lane>, ApiError> {
    let id = resource_id(path).await?;
    state
        .net
        .get_lane(LaneID::from_usize(id))
        .cloned()
        .map(Json)
        .ok_or_else(|| ApiError::not_found("lane", id))
}
async fn edge(
    State(state): State<ApiState>,
    path: Result<Path<usize>, PathRejection>,
) -> Result<Json<Edge>, ApiError> {
    let id = resource_id(path).await?;
    state
        .net
        .get_edge(EdgeID::from_usize(id))
        .cloned()
        .map(Json)
        .ok_or_else(|| ApiError::not_found("edge", id))
}
async fn junction(
    State(state): State<ApiState>,
    path: Result<Path<usize>, PathRejection>,
) -> Result<Json<Junction>, ApiError> {
    let id = resource_id(path).await?;
    state
        .net
        .get_junction(JunctionID::from_usize(id))
        .cloned()
        .map(Json)
        .ok_or_else(|| ApiError::not_found("junction", id))
}

async fn vehicle(
    State(state): State<ApiState>,
    path: Result<Path<usize>, PathRejection>,
) -> Result<Json<Vehicle>, ApiError> {
    let id = resource_id(path).await?;
    let _simulation_gate = state
        .simulation_gate
        .try_read()
        .map_err(|_| ApiError::conflict("Vehicles cannot be read while a run is in progress."))?;
    let _gate = state.gate.lock().await;
    state
        .simulator
        .lock()
        .unwrap()
        .get_vehicle(VehicleID::from_usize(id))
        .cloned()
        .map(Json)
        .ok_or_else(|| ApiError::not_found("vehicle", id))
}

#[derive(Deserialize)]
struct RunQuery {
    steps: Option<usize>,
}
#[derive(Serialize)]
#[serde(tag = "event", rename_all = "camelCase")]
enum RunEvent {
    Accepted { requested_steps: usize },
    Completed(RunStat),
    Failed { detail: String },
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunStat {
    pub start_step: usize,
    pub end_step: usize,
    pub completed_steps: usize,
    pub has_ended: bool,
    pub simulated_duration_millis: usize,
    pub script_duration_millis: usize,
    pub sumo_duration_millis: usize,
    pub total_duration_millis: usize,
}
async fn run_simulation(
    State(state): State<ApiState>,
    query: Result<Query<RunQuery>, QueryRejection>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, ApiError> {
    let steps = query
        .map_err(|_| {
            ApiError::bad_request(
                "The optional query parameter `steps` must be a non-negative integer.",
            )
        })?
        .steps
        .unwrap_or(1);
    if steps == 0 {
        return Err(ApiError::bad_request("`steps` must be greater than zero."));
    }
    let (event_tx, event_rx) = tokio::sync::mpsc::channel(4);
    tokio::spawn(async move {
        let _simulation_gate = state.simulation_gate.write().await;
        let _guard = state.gate.lock().await;
        if state.compute_tx.send(steps).is_err() {
            let _ = event_tx
                .send(Ok(Event::default()
                    .event("failed")
                    .json_data(RunEvent::Failed {
                        detail: "The simulation worker is unavailable.".to_owned(),
                    })
                    .unwrap()))
                .await;
            return;
        }
        let _ = event_tx
            .send(Ok(Event::default()
                .event("accepted")
                .json_data(RunEvent::Accepted {
                    requested_steps: steps,
                })
                .unwrap()))
            .await;
        match state.compute_rx.lock().await.recv().await {
            Some(Ok(run_stat)) => {
                let _ = event_tx
                    .send(Ok(Event::default()
                        .event("completed")
                        .json_data(RunEvent::Completed(run_stat))
                        .unwrap()))
                    .await;
            }
            Some(Err(detail)) => {
                let _ = event_tx
                    .send(Ok(Event::default()
                        .event("failed")
                        .json_data(RunEvent::Failed { detail })
                        .unwrap()))
                    .await;
            }
            None => {
                let _ = event_tx
                    .send(Ok(Event::default()
                        .event("failed")
                        .json_data(RunEvent::Failed {
                            detail: "The simulation worker closed before reporting completion."
                                .to_owned(),
                        })
                        .unwrap()))
                    .await;
            }
        }
    });
    Ok(Sse::new(ReceiverStream::new(event_rx)))
}

async fn openapi_document() -> Response {
    (
        [(
            header::CONTENT_TYPE,
            "application/vnd.oai.openapi+json;version=3.1",
        )],
        include_str!("../openapi.json"),
    )
        .into_response()
}
