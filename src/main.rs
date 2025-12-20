use rust_embed::RustEmbed;
use warp::Filter;
use std::net::SocketAddr;

#[derive(RustEmbed)]
#[folder = "web/"]
struct Asset;

#[tokio::main]
async fn main() {
    let routes = warp::path::tail().map(|tail: warp::path::Tail| {
        let path = tail.as_str();
        let asset_path = if path == "" { "index.html" } else { path };

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
    });

    // Bind to port 0 to get a random free port
    let addr: SocketAddr = ([127, 0, 0, 1], 0).into();

    println!("Starting server...");
    let server = warp::serve(routes).bind_ephemeral(addr);
    
    // We need to spawn the server and get the bound address
    // warp's run/bind_ephemeral is a bit tricky to get the address back if not using bind_with_graceful_shutdown
    // Let's us bind_ephemeral which returns (SocketAddr, Future)
    
    let (addr, server) = server;
    println!("Listening on http://{}", addr);

    // Open browser
    let url = format!("http://{}", addr);
    if let Err(e) = webbrowser::open(&url) {
        eprintln!("Failed to open browser: {}", e);
    }

    server.await;
}
