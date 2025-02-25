use anyhow::Result;
use iroh::Endpoint;

#[tokio::main]
async fn main() -> Result<()> {
    println!("Testing iroh API");
    
    // Try to create an endpoint
    println!("Creating endpoint...");
    let endpoint = Endpoint::builder()
        .discovery_n0()
        .bind()
        .await?;
    println!("Endpoint created successfully!");
    
    // Get the node ID
    let node_id = endpoint.node_id().to_string();
    println!("Node ID: {}", node_id);
    
    // Print the version of iroh
    println!("Iroh version: {}", env!("CARGO_PKG_VERSION"));
    
    Ok(())
} 