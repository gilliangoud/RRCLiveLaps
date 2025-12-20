use serde::Deserialize;
use tokio::net::TcpListener;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::broadcast;
use std::sync::{atomic::{AtomicBool, Ordering}, Arc};
use crate::messages::{WsMessage, Passing};

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
struct JsonPassingInner {
    Transponder: String,
    Hits: Option<i64>,
    RSSI: Option<i64>,
    Battery: Option<f64>,
    Temperature: Option<f64>,
    LoopID: Option<i64>,
    Channel: Option<i64>,
    InternalData: Option<String>,
    PassingNo: Option<i64>,
    UTCTime: String, // "2024-01-12T09:06:35.944Z"
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
struct JsonPassingWrapper {
    Passing: JsonPassingInner,
    Time: Option<f64>,
}

pub async fn run_server(tx: broadcast::Sender<WsMessage>, port: u16, is_connected: Arc<AtomicBool>, debug: bool) {
    let addr = format!("0.0.0.0:{}", port);
    let listener = match TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Failed to bind JSON server to {}: {}", addr, e);
            return;
        }
    };

    println!("JSON Server listening on {}", addr);

    loop {
        let (socket, addr) = match listener.accept().await {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Error accepting connection: {}", e);
                continue;
            }
        };

        println!("New JSON client connection from {}", addr);
        
        // Mark as connected when a client connects
        is_connected.store(true, Ordering::SeqCst);
        let _ = tx.send(WsMessage::Status { event: "connected".to_string() });

        let tx = tx.clone();
        let is_connected = is_connected.clone();

        tokio::spawn(async move {
            let reader = BufReader::new(socket);
            let mut lines = reader.lines();

            while let Ok(Some(line)) = lines.next_line().await {
                if line.trim().is_empty() {
                    continue;
                }

                if debug {
                    println!("DEBUG Input: {}", line);
                }

                match serde_json::from_str::<JsonPassingWrapper>(&line) {
                    Ok(wrapper) => {
                        let time_val = wrapper.Time;
                        let inner = wrapper.Passing;
                        
                        // Parse UTCTime to date and time
                        // Format: "2024-01-12T09:06:35.944Z"
                        let (mut date_str, mut time_str) = if let Some((d, t)) = inner.UTCTime.split_once('T') {
                            (d.to_string(), t.trim_end_matches('Z').to_string())
                        } else {
                            ("".to_string(), "".to_string())
                        };

                        // Always prioritize Time object if available
                        if let Some(seconds_since_midnight) = time_val {
                            // Calculate time from seconds
                            let seconds = seconds_since_midnight as u32;
                            let millis = ((seconds_since_midnight - seconds as f64) * 1000.0) as u32;
                            let hours = seconds / 3600;
                            let minutes = (seconds % 3600) / 60;
                            let secs = seconds % 60;
                            
                            time_str = format!("{:02}:{:02}:{:02}.{:03}", hours, minutes, secs, millis);
                            
                            // If date_str is invalid or empty, use current date
                            if date_str == "0001-01-01" || date_str.is_empty() {
                                let now = chrono::Local::now();
                                date_str = now.format("%Y-%m-%d").to_string();
                            }
                        }

                        let full_iso_date = if !date_str.is_empty() && !time_str.is_empty() {
                            format!("{}T{}", date_str, time_str)
                        } else {
                            inner.UTCTime.clone()
                        };

                        let passing = Passing {
                            passing_number: inner.PassingNo.map(|v| v as u32).unwrap_or(0),
                            transponder: inner.Transponder,
                            date: full_iso_date.clone(),
                            time: time_str.clone(),
                            rtc_time: full_iso_date,
                            strength: inner.RSSI.map(|v| v as u32).unwrap_or(0),
                            tran_code: inner.InternalData.unwrap_or_default(),
                            noise: 0, 
                            hits: inner.Hits.map(|v| v as u32).unwrap_or(0),
                        };

                        if debug {
                             println!("JSON Passing: {:?}", passing);
                        } else {
                             // println!("JSON Passing received");
                        }

                        if let Err(e) = tx.send(WsMessage::Passing(passing)) {
                            eprintln!("Error broadcasting passing: {}", e);
                        }
                    }
                    Err(e) => {
                        eprintln!("Error parsing JSON: {}. Data: {}", e, line);
                    }
                }
            }

            println!("JSON client {} disconnected", addr);
            is_connected.store(false, Ordering::SeqCst);
            let _ = tx.send(WsMessage::Status { event: "disconnected".to_string() });
        });
    }
}
