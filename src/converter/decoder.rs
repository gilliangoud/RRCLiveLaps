// use chrono::{NaiveDate, NaiveTime, NaiveDateTime};
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::sync::broadcast;
use tokio::time::interval;
use tokio_util::codec::{Framed, LinesCodec};
use std::sync::{atomic::{AtomicBool, Ordering}, Arc};

use crate::messages::{WsMessage, Passing};

// Remove local definitions
/*
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Passing { ... }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum WsMessage { ... }
*/

pub struct Decoder {
    ip: IpAddr,
    port: u16,
}

impl Decoder {
    pub fn new(ip: IpAddr, port: u16) -> Self {
        Self { ip, port }
    }

    pub async fn run(&self, tx: broadcast::Sender<WsMessage>, is_connected: Arc<AtomicBool>) {
        println!("Connecting to decoder at {}:{}", self.ip, self.port);
        match TcpStream::connect((self.ip, self.port)).await {
            Ok(socket) => {
                println!("Connected to decoder");
                
                // Status: Connected
                is_connected.store(true, Ordering::SeqCst);
                let _ = tx.send(WsMessage::Status { event: "connected".to_string() });

                if let Err(e) = self.handle_connection(socket, &tx).await {
                    eprintln!("Connection error: {}", e);
                }
            }
            Err(e) => {
                eprintln!("Failed to connect: {}", e);
            }
        }

        // Status: Disconnected
        // Only send if we were previously connected (or just ensure state is false)
        if is_connected.load(Ordering::SeqCst) {
            is_connected.store(false, Ordering::SeqCst);
            let _ = tx.send(WsMessage::Status { event: "disconnected".to_string() });
        }
    }

    async fn handle_connection(
        &self,
        socket: TcpStream,
        tx: &broadcast::Sender<WsMessage>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut framed = Framed::new(socket, LinesCodec::new());

        // Initialize protocol
        framed.send("SETPROTOCOL;2.0").await?;
        
        if let Some(line) = framed.next().await {
            let msg = line?;
            if msg != "SETPROTOCOL;2.0" {
                eprintln!("Unexpected response to SETPROTOCOL: {}", msg);
            } else {
                println!("Protocol set to 2.0");
            }
        } else {
            return Err("Connection closed during initialization".into());
        }

        framed.send("SETPUSHPASSINGS;1;1").await?;

        if let Some(line) = framed.next().await {
            let msg = line?;
            if msg != "SETPUSHPASSINGS;1" {
                eprintln!("Unexpected response to SETPUSHPASSINGS: {}", msg);
            } else {
                println!("Push passings enabled");
            }
        } else {
            return Err("Connection closed during initialization".into());
        }

        // Ping interval
        let mut ping_interval = interval(Duration::from_secs(30));

        loop {
            tokio::select! {
                line = framed.next() => {
                    match line {
                        Some(Ok(msg)) => {
                            self.process_message(&msg, tx);
                        }
                        Some(Err(e)) => return Err(Box::new(e)),
                        None => return Err("Connection closed".into()),
                    }
                }
                _ = ping_interval.tick() => {
                    framed.send("PING").await?;
                }
            }
        }
    }

    fn process_message(&self, msg: &str, tx: &broadcast::Sender<WsMessage>) {
        // println!("Received: {}", msg);
        let parts: Vec<&str> = msg.split(';').collect();
        if parts.is_empty() {
            return;
        }

        match parts[0] {
            "#P" => {
                // Format: #P;PassingNo;Transponder;Date;Time;EventID;Hits;MaxRSSI;InternalData;IsActive;Channel;LoopID;LoopIDWakeup;Battery;Temperature;InternalActiveData;BoxTemp;BoxReaderID
                // Note: Fields might be empty or missing depending on device.
                // We'll try to get as many as possible, defaulting to empty string.
                
                let get_part = |idx: usize| -> String {
                    parts.get(idx).unwrap_or(&"").to_string()
                };

                if parts.len() >= 5 {
                    let passing_number = get_part(1);
                    let transponder = get_part(2);
                    let date_str = get_part(3);
                    let time_str = get_part(4);
                    
                    // Combine date and time for ISO string
                    let iso_date = format!("{}T{}", date_str, time_str);

                    let passing = Passing {
                        passing_number: passing_number.parse().unwrap_or(0),
                        transponder: transponder,
                        date: iso_date,
                        time: time_str.clone(),
                        rtc_time: format!("{}T{}", date_str, time_str), // Using rtc_time as iso_date for now? or add field
                        strength: get_part(7).parse().unwrap_or(0), // max_rssi
                        tran_code: get_part(8), // internal_data?
                        noise: 0,
                        hits: get_part(6).parse().unwrap_or(0),
                    };
                    
                    if let Err(e) = tx.send(WsMessage::Passing(passing)) {
                        eprintln!("Error broadcasting passing: {}. Original data: {}", e, msg);
                    }
                } else {
                    eprintln!("Error processing passing: Insufficient data parts. Original data: {}", msg);
                }
            }
            "PING" => {
                // Ignore
            }
            _ => {
                // Ignore other messages
            }
        }
    }
}
