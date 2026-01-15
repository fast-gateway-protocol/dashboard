//! REST API endpoints for the FGP Dashboard.

use axum::{
    extract::Path,
    http::StatusCode,
    response::{Html, IntoResponse, Json},
};
use serde::{Deserialize, Serialize};
use std::fs;

/// Service status information
#[derive(Serialize, Deserialize)]
pub struct ServiceInfo {
    pub name: String,
    pub status: String,
    pub version: Option<String>,
    pub uptime_seconds: Option<u64>,
    pub socket_path: String,
}

/// API response wrapper
#[derive(Serialize)]
pub struct ApiResponse<T> {
    pub ok: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn success(data: T) -> Json<Self> {
        Json(Self {
            ok: true,
            data: Some(data),
            error: None,
        })
    }

    pub fn error(message: &str) -> Json<Self> {
        Json(Self {
            ok: false,
            data: None,
            error: Some(message.to_string()),
        })
    }
}

/// List all installed services and their status
pub async fn list_services() -> impl IntoResponse {
    let services_dir = fgp_daemon::fgp_services_dir();

    if !services_dir.exists() {
        return ApiResponse::<Vec<ServiceInfo>>::success(vec![]);
    }

    let mut services = Vec::new();

    if let Ok(entries) = fs::read_dir(&services_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();

            let socket_path = fgp_daemon::service_socket_path(&name);
            let socket_str = socket_path.to_string_lossy().to_string();

            let (status, version, uptime) = if socket_path.exists() {
                match fgp_daemon::FgpClient::new(&socket_path) {
                    Ok(client) => match client.health() {
                        Ok(response) if response.ok => {
                            let result = response.result.unwrap_or_default();
                            let version = result["version"].as_str().map(|s| s.to_string());
                            let uptime = result["uptime_seconds"].as_u64();
                            let status = result["status"].as_str().unwrap_or("running").to_string();
                            (status, version, uptime)
                        }
                        _ => ("not_responding".to_string(), None, None),
                    },
                    Err(_) => ("socket_error".to_string(), None, None),
                }
            } else {
                ("stopped".to_string(), None, None)
            };

            services.push(ServiceInfo {
                name,
                status,
                version,
                uptime_seconds: uptime,
                socket_path: socket_str,
            });
        }
    }

    // Sort by name
    services.sort_by(|a, b| a.name.cmp(&b.name));

    ApiResponse::success(services)
}

/// Get detailed health info for a specific service
pub async fn service_health(Path(service): Path<String>) -> impl IntoResponse {
    let socket_path = fgp_daemon::service_socket_path(&service);

    if !socket_path.exists() {
        return (
            StatusCode::NOT_FOUND,
            ApiResponse::<serde_json::Value>::error(&format!(
                "Service '{}' is not running",
                service
            )),
        );
    }

    match fgp_daemon::FgpClient::new(&socket_path) {
        Ok(client) => match client.health() {
            Ok(response) if response.ok => {
                let result = response.result.unwrap_or_default();
                (StatusCode::OK, ApiResponse::success(result))
            }
            Ok(response) => {
                let error = response.error.map(|e| e.message).unwrap_or_default();
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    ApiResponse::<serde_json::Value>::error(&error),
                )
            }
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                ApiResponse::<serde_json::Value>::error(&e.to_string()),
            ),
        },
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            ApiResponse::<serde_json::Value>::error(&e.to_string()),
        ),
    }
}

/// Start a service
pub async fn start_service(Path(service): Path<String>) -> impl IntoResponse {
    match fgp_daemon::start_service(&service) {
        Ok(()) => (
            StatusCode::OK,
            ApiResponse::success(serde_json::json!({
                "message": format!("Service '{}' started", service)
            })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            ApiResponse::<serde_json::Value>::error(&e.to_string()),
        ),
    }
}

/// Stop a service
pub async fn stop_service(Path(service): Path<String>) -> impl IntoResponse {
    match fgp_daemon::stop_service(&service) {
        Ok(()) => (
            StatusCode::OK,
            ApiResponse::success(serde_json::json!({
                "message": format!("Service '{}' stopped", service)
            })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            ApiResponse::<serde_json::Value>::error(&e.to_string()),
        ),
    }
}

/// Serve the static HTML dashboard
pub async fn serve_dashboard() -> Html<&'static str> {
    Html(DASHBOARD_HTML)
}

/// Embedded HTML dashboard
const DASHBOARD_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>FGP Dashboard</title>
    <style>
        * {
            box-sizing: border-box;
            margin: 0;
            padding: 0;
        }
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, sans-serif;
            background: #0f0f0f;
            color: #e0e0e0;
            min-height: 100vh;
            padding: 2rem;
        }
        .container {
            max-width: 1200px;
            margin: 0 auto;
        }
        header {
            display: flex;
            justify-content: space-between;
            align-items: center;
            margin-bottom: 2rem;
            padding-bottom: 1rem;
            border-bottom: 1px solid #333;
        }
        h1 {
            font-size: 1.5rem;
            font-weight: 600;
            color: #fff;
        }
        .refresh-info {
            font-size: 0.85rem;
            color: #666;
        }
        .services-grid {
            display: grid;
            grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
            gap: 1rem;
        }
        .service-card {
            background: #1a1a1a;
            border: 1px solid #333;
            border-radius: 8px;
            padding: 1.25rem;
            transition: border-color 0.2s;
        }
        .service-card:hover {
            border-color: #555;
        }
        .service-header {
            display: flex;
            justify-content: space-between;
            align-items: center;
            margin-bottom: 1rem;
        }
        .service-name {
            font-weight: 600;
            font-size: 1.1rem;
            color: #fff;
        }
        .status-badge {
            display: inline-flex;
            align-items: center;
            gap: 0.5rem;
            padding: 0.25rem 0.75rem;
            border-radius: 9999px;
            font-size: 0.8rem;
            font-weight: 500;
        }
        .status-badge.running {
            background: rgba(34, 197, 94, 0.15);
            color: #22c55e;
        }
        .status-badge.stopped {
            background: rgba(100, 100, 100, 0.15);
            color: #888;
        }
        .status-badge.error {
            background: rgba(239, 68, 68, 0.15);
            color: #ef4444;
        }
        .status-badge.unhealthy {
            background: rgba(245, 158, 11, 0.15);
            color: #f59e0b;
        }
        .status-dot {
            width: 8px;
            height: 8px;
            border-radius: 50%;
            animation: pulse 2s infinite;
        }
        .status-dot.running { background: #22c55e; }
        .status-dot.stopped { background: #888; animation: none; }
        .status-dot.error { background: #ef4444; }
        .status-dot.unhealthy { background: #f59e0b; }
        @keyframes pulse {
            0%, 100% { opacity: 1; }
            50% { opacity: 0.5; }
        }
        .service-details {
            font-size: 0.85rem;
            color: #888;
            margin-bottom: 1rem;
        }
        .service-details span {
            display: block;
            margin-bottom: 0.25rem;
        }
        .service-actions {
            display: flex;
            gap: 0.5rem;
        }
        .btn {
            flex: 1;
            padding: 0.5rem 1rem;
            border: none;
            border-radius: 6px;
            font-size: 0.85rem;
            font-weight: 500;
            cursor: pointer;
            transition: all 0.2s;
        }
        .btn:disabled {
            opacity: 0.5;
            cursor: not-allowed;
        }
        .btn-start {
            background: #22c55e;
            color: #000;
        }
        .btn-start:hover:not(:disabled) {
            background: #16a34a;
        }
        .btn-stop {
            background: #ef4444;
            color: #fff;
        }
        .btn-stop:hover:not(:disabled) {
            background: #dc2626;
        }
        .loading {
            text-align: center;
            padding: 3rem;
            color: #666;
        }
        .empty-state {
            text-align: center;
            padding: 3rem;
            color: #666;
        }
    </style>
</head>
<body>
    <div class="container">
        <header>
            <h1>FGP Dashboard</h1>
            <span class="refresh-info" id="refresh-info">Refreshing...</span>
        </header>
        <div id="app" class="services-grid">
            <div class="loading">Loading services...</div>
        </div>
    </div>
    <script>
        const API_BASE = '';
        let services = [];

        function formatUptime(seconds) {
            if (!seconds) return '-';
            if (seconds < 60) return `${seconds}s`;
            if (seconds < 3600) return `${Math.floor(seconds / 60)}m ${seconds % 60}s`;
            if (seconds < 86400) return `${Math.floor(seconds / 3600)}h ${Math.floor((seconds % 3600) / 60)}m`;
            return `${Math.floor(seconds / 86400)}d ${Math.floor((seconds % 86400) / 3600)}h`;
        }

        function getStatusClass(status) {
            if (status === 'running' || status === 'healthy') return 'running';
            if (status === 'stopped') return 'stopped';
            if (status === 'unhealthy' || status === 'degraded') return 'unhealthy';
            return 'error';
        }

        function renderServices() {
            const app = document.getElementById('app');

            if (services.length === 0) {
                app.innerHTML = '<div class="empty-state">No services installed</div>';
                return;
            }

            app.innerHTML = services.map(service => {
                const statusClass = getStatusClass(service.status);
                const isRunning = statusClass === 'running';

                return `
                    <div class="service-card">
                        <div class="service-header">
                            <span class="service-name">${service.name}</span>
                            <span class="status-badge ${statusClass}">
                                <span class="status-dot ${statusClass}"></span>
                                ${service.status}
                            </span>
                        </div>
                        <div class="service-details">
                            <span>Version: ${service.version || '-'}</span>
                            <span>Uptime: ${formatUptime(service.uptime_seconds)}</span>
                        </div>
                        <div class="service-actions">
                            <button class="btn btn-start"
                                    onclick="startService('${service.name}')"
                                    ${isRunning ? 'disabled' : ''}>
                                Start
                            </button>
                            <button class="btn btn-stop"
                                    onclick="stopService('${service.name}')"
                                    ${!isRunning ? 'disabled' : ''}>
                                Stop
                            </button>
                        </div>
                    </div>
                `;
            }).join('');
        }

        async function fetchServices() {
            try {
                const response = await fetch(`${API_BASE}/api/services`);
                const result = await response.json();
                if (result.ok) {
                    services = result.data;
                    renderServices();
                }
            } catch (error) {
                console.error('Failed to fetch services:', error);
            }
            updateRefreshInfo();
        }

        async function startService(name) {
            try {
                const response = await fetch(`${API_BASE}/api/start/${name}`, { method: 'POST' });
                const result = await response.json();
                if (!result.ok) {
                    alert(`Failed to start ${name}: ${result.error}`);
                }
                await fetchServices();
            } catch (error) {
                alert(`Failed to start ${name}: ${error.message}`);
            }
        }

        async function stopService(name) {
            try {
                const response = await fetch(`${API_BASE}/api/stop/${name}`, { method: 'POST' });
                const result = await response.json();
                if (!result.ok) {
                    alert(`Failed to stop ${name}: ${result.error}`);
                }
                await fetchServices();
            } catch (error) {
                alert(`Failed to stop ${name}: ${error.message}`);
            }
        }

        function updateRefreshInfo() {
            const now = new Date().toLocaleTimeString();
            document.getElementById('refresh-info').textContent = `Last updated: ${now}`;
        }

        // Initial fetch
        fetchServices();

        // Auto-refresh every 5 seconds
        setInterval(fetchServices, 5000);
    </script>
</body>
</html>
"#;
