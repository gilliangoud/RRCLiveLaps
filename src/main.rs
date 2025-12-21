use rust_embed::RustEmbed;
use warp::Filter;
use std::net::SocketAddr;
use std::sync::{atomic::AtomicBool, Arc};
use tokio::sync::broadcast;

mod messages;
mod json_server;
mod config;
mod ws_handler;

mod converter {
    pub mod decoder;
}
mod usb {
    pub mod decoder;
}

use messages::WsMessage;

#[derive(RustEmbed)]
#[folder = "web/"]
struct Asset;

use tokio::sync::mpsc;
use std::path::{Path, PathBuf};

fn api_filters(config_path: PathBuf, mapping_path: PathBuf, shutdown_tx: mpsc::Sender<()>) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    let config_path = Arc::new(config_path);
    let mapping_path = Arc::new(mapping_path);
    let p1 = config_path.clone();
    let p2 = config_path.clone();
    let m1 = mapping_path.clone();
    
    let api = warp::path("api");

    let mapping_route = api
        .and(warp::path("mapping"))
        .and(warp::post())
        .and(warp::body::json())
        .map(move |mapping: std::collections::HashMap<String, String>| {
            println!("Received new mapping update");
            match std::fs::File::create(m1.as_path()) {
                Ok(file) => {
                    match serde_json::to_writer_pretty(file, &mapping) {
                        Ok(_) => warp::reply::json(&"Success"),
                        Err(e) => {
                            eprintln!("Failed to write JSON: {}", e);
                            warp::reply::json(&"Error writing JSON")
                        }
                    }
                },
                Err(e) => {
                    eprintln!("Failed to create mapping.json: {}", e);
                    warp::reply::json(&"Error creating file")
                }
            }
        });

    let shutdown_tx = warp::any().map(move || shutdown_tx.clone());

    let config_route = api
        .and(warp::path("config"))
        .and(
            warp::get().map(move || {
                let config = config::load_config(p1.to_str().unwrap_or("config.json"));
                warp::reply::json(&config)
            })
            .or(
                warp::post()
                .and(warp::body::json())
                .and(shutdown_tx)
                .map(move |config: config::Config, tx: mpsc::Sender<()>| {
                    config::save_config(p2.to_str().unwrap_or("config.json"), &config);
                    // Spawn a task to send shutdown signal after a short delay
                    // to allow the response to be sent
                    tokio::spawn(async move {
                        tokio::time::sleep(std::time::Duration::from_millis(500)).await; 
                        let _ = tx.send(()).await;
                    });
                    warp::reply::json(&"Success")
                })
            )
        );

    mapping_route.or(config_route)
}

fn static_filters(mapping_path: PathBuf) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    let mapping_path = Arc::new(mapping_path);

    warp::get().and(warp::path::tail()).map(move |tail: warp::path::Tail| {
        let path = tail.as_str();
        let asset_path = if path == "" { "index.html" } else { path };

        if asset_path == "mapping.json" && mapping_path.exists() {
             match std::fs::read(mapping_path.as_path()) {
                Ok(content) => {
                     // println!("Serving external mapping.json");
                     return warp::http::Response::builder()
                        .header("content-type", "application/json")
                        .body(content)
                        .unwrap_or_else(|_| warp::http::Response::new(vec![]));
                },
                Err(e) => eprintln!("Failed to read local mapping.json: {}", e),
             }
        }

        match Asset::get(asset_path) {
            Some(content) => {
                let mime = mime_guess::from_path(asset_path).first_or_octet_stream();
                warp::http::Response::builder()
                    .header("content-type", mime.as_ref())
                    .body(content.data.into_owned())
                    .unwrap_or_else(|_| warp::http::Response::new(vec![]))
            }
            None => {
                warp::http::Response::builder()
                    .header("content-type", "text/plain")
                    .body(vec![])
                    .unwrap_or_else(|_| warp::http::Response::new(vec![]))
            }
        }
    })
}

fn get_config_paths() -> (PathBuf, PathBuf) {
    let mut exe_dir = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("."));
    if exe_dir.file_name().is_some() {
        exe_dir.pop(); // Remove executable name
    }
    
    let config_path = exe_dir.join("config.json");
    let mapping_path = exe_dir.join("mapping.json");
    
    (config_path, mapping_path)
}

#[tokio::main]
async fn main() {
    let (config_path, mapping_path) = get_config_paths();
    
    println!("Using config file: {:?}", config_path);
    println!("Using mapping file: {:?}", mapping_path);

    // Load Configuration
    let config = config::load_config(config_path.to_str().unwrap_or("config.json"));
    println!("Loaded config: {:?}", config);

    // Initialize Channels and State
    let (tx, _rx) = broadcast::channel::<WsMessage>(100);
    let is_connected = Arc::new(AtomicBool::new(false));
    
    // Spawn Decoder Task based on Mode
    let tx_clone = tx.clone();
    let is_connected_clone = is_connected.clone();
    let config_clone = config.clone();

    tokio::spawn(async move {
        match config_clone.mode {
            config::AppMode::Tcp { host, port } => {
                println!("Starting in TCP Mode: {}:{}", host, port);
                let ip = host.parse().expect("Invalid IP address");
                let decoder = converter::decoder::Decoder::new(ip, port);
                decoder.run(tx_clone, is_connected_clone).await;
            },
            config::AppMode::Usb { port_path } => {
                println!("Starting in USB Mode: {}", port_path);
                let usb_box = usb::decoder::UsbBox::new(port_path, 10);
                usb_box.run(tx_clone, is_connected_clone).await;
            },
            config::AppMode::TcpServer { port } => {
                println!("Starting in TCP Server Mode on port {}", port);
                json_server::run_server(tx_clone, port, is_connected_clone, false).await;
            }
        }
    });

    // Shutdown channel
    let (shutdown_tx, mut shutdown_rx) = mpsc::channel(1);

    // Setup Routes
    let api = api_filters(config_path, mapping_path.clone(), shutdown_tx);
    // WS route needs tx and is_connected
    let ws = ws_handler::ws_routes(tx, is_connected);
    let static_files = static_filters(mapping_path);

    let routes = api.or(ws).or(static_files);

    // Bind to port 0 to get a random free port
    let addr: SocketAddr = ([127, 0, 0, 1], 8080).into();

    println!("Starting server...");
    let server = warp::serve(routes).bind_with_graceful_shutdown(addr, async move {
        shutdown_rx.recv().await;
        println!("Shutting down server...");
    });
    
    let (addr, server) = server;
    println!("Listening on http://{}", addr);

    // Open browser
    let url = format!("http://{}", addr);
    if let Err(e) = webbrowser::open(&url) {
        eprintln!("Failed to open browser: {}", e);
    }

    server.await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_save_mapping() {
        let (tx, _) = mpsc::channel(1);
        let mapping_path = PathBuf::from("test_mapping.json");
        let config_path = PathBuf::from("test_config_dummy.json");
        
        let filter = api_filters(config_path, mapping_path.clone(), tx);

        let mut map = HashMap::new();
        map.insert("001".to_string(), "Test Driver".to_string());

        let resp = warp::test::request()
            .method("POST")
            .path("/api/mapping")
            .json(&map)
            .reply(&filter)
            .await;

        assert_eq!(resp.status(), 200);

        // Check if file exists and has content
        let content = std::fs::read_to_string(&mapping_path).expect("Failed to read file");
        assert!(content.contains("Test Driver"));
        
        // Clean up
        let _ = std::fs::remove_file(mapping_path);
    }

    #[tokio::test]
    async fn test_save_config() {
        let test_file = PathBuf::from("test_config.json");
        let mapping_dummy = PathBuf::from("test_mapping_dummy.json");
        
        let (tx, mut rx) = mpsc::channel(1);
        let filter = api_filters(test_file.clone(), mapping_dummy, tx);

        let new_config = config::Config {
            mode: config::AppMode::Tcp {
                host: "10.0.0.1".to_string(),
                port: 1234
            }
        };

        let resp = warp::test::request()
            .method("POST")
            .path("/api/config")
            .json(&new_config)
            .reply(&filter)
            .await;

        assert_eq!(resp.status(), 200);

        // Verify file content
        let content = std::fs::read_to_string(&test_file).expect("Failed to read config.json");
        assert!(content.contains("10.0.0.1"));
        
        // Cleanup
        let _ = std::fs::remove_file(test_file);
        
        // Verify shutdown signal was sent
        let msg = rx.recv().await;
        assert!(msg.is_some());
    }
}
