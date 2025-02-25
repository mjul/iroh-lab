use anyhow::Result;
use iroh::{Endpoint, protocol::Router};
use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::main]
async fn main() -> Result<()> {
    println!("Testing iroh API");
    
    // Try to create an endpoint
    println!("Creating endpoint...");
    let endpoint = Endpoint::new()?;
    println!("Endpoint created successfully!");
    
    // Get the node ID
    let node_id = endpoint.node_id().to_string();
    println!("Node ID: {}", node_id);
    
    // Create a router
    println!("Creating router...");
    let router = Router::default();
    println!("Router created successfully!");
    
    // Print the version of iroh
    println!("Iroh version: {}", env!("CARGO_PKG_VERSION"));
    
    Ok(())
} 