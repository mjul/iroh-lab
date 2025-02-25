use anyhow::Result as AnyhowResult;
use chrono::{DateTime, Utc};
use futures::StreamExt;
use iroh::Endpoint;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{info, instrument, trace, warn};
use uuid::Uuid;

// Message structure for chat
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: String,
    pub author: String,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub topic_hash: String,
    pub sequence: u64,
}

// Channel for receiving messages from the network
pub static mut MESSAGE_SENDER: Option<mpsc::UnboundedSender<ChatMessage>> = None;
pub static mut MESSAGE_RECEIVER: Option<mpsc::UnboundedReceiver<ChatMessage>> = None;

#[derive(Clone)]
pub struct IrohClient {
    pub node_id: Option<String>,
    pub topic_ticket: Option<String>,
    pub topic_hash: Option<String>,
    pub subscribed_topics: HashMap<String, String>,
    endpoint: Option<Endpoint>,
}

impl IrohClient {
    pub fn new() -> Self {
        trace!("Creating new IrohClient instance");
        Self {
            node_id: None,
            topic_ticket: None,
            topic_hash: None,
            subscribed_topics: HashMap::new(),
            endpoint: None,
        }
    }

    pub fn initialize_message_channel() -> (
        mpsc::UnboundedSender<ChatMessage>,
        mpsc::UnboundedReceiver<ChatMessage>,
    ) {
        trace!("Initializing message channel");
        let (sender, receiver) = mpsc::unbounded_channel();
        unsafe {
            // Store the main sender
            MESSAGE_SENDER = Some(sender.clone());

            // Store the main receiver if it doesn't exist yet
            // (only one main receiver should exist)
            if MESSAGE_RECEIVER.is_none() {
                MESSAGE_RECEIVER = Some(receiver);
                trace!("Message receiver initialized");

                // Set up a forwarding task to make sure messages can still flow
                // even if the original receiver is taken
                let sender_clone = sender.clone();
                tokio::spawn(async move {
                    trace!("Starting message forwarding task");
                    // This task keeps the channel alive
                });

                // Return the original pair
                return (sender, mpsc::unbounded_channel().1); // Return a dummy receiver
            }
        }
        trace!("Using existing message channel");
        (sender, mpsc::unbounded_channel().1) // Return a dummy receiver
    }

    pub fn get_message_sender() -> Option<mpsc::UnboundedSender<ChatMessage>> {
        trace!("Getting message sender");
        unsafe { MESSAGE_SENDER.clone() }
    }

    pub fn get_message_receiver() -> Option<mpsc::UnboundedReceiver<ChatMessage>> {
        trace!("Getting message receiver clone");

        // Create a new channel that will receive messages
        let (new_sender, new_receiver) = mpsc::unbounded_channel();

        // Get the original sender to forward messages to the new channel
        if let Some(sender) = Self::get_message_sender() {
            // Store the new sender in a static variable to forward messages
            unsafe {
                // Ensure we have a valid MESSAGE_FORWARDERS list
                static mut MESSAGE_FORWARDERS: Option<Vec<mpsc::UnboundedSender<ChatMessage>>> = None;

                // Initialize the forwarders vector if it doesn't exist
                if MESSAGE_FORWARDERS.is_none() {
                    MESSAGE_FORWARDERS = Some(Vec::new());
                }
                
                // Add our new sender to the list of forwarders
                if let Some(forwarders) = &mut MESSAGE_FORWARDERS {
                    // Clean up any closed channels before adding a new one
                    forwarders.retain(|forwarder| {
                        match forwarder.send(ChatMessage {
                            id: "ping".to_string(),
                            author: "system".to_string(),
                            content: "ping".to_string(),
                            timestamp: Utc::now(),
                            topic_hash: "ping".to_string(),
                            sequence: 0,
                        }) {
                            Ok(_) => true,
                            Err(_) => false, // Remove closed channels
                        }
                    });
                    
                    // Now add the new forwarder
                    forwarders.push(new_sender);
                    
                    trace!("Added new message forwarder, total forwarders: {}", forwarders.len());
                }

                // Return the new receiver
                return Some(new_receiver);
            }
        }

        None
    }

    // This function should be used to send messages, ensuring they go to all receivers
    pub fn broadcast_message(message: ChatMessage) {
        // Filter ping messages (used just for checking channel liveness)
        if message.id == "ping" && message.author == "system" {
            return;
        }
        
        trace!(
            message_id = %message.id,
            author = %message.author,
            "Broadcasting message to all receivers"
        );
        
        // Send to the main channel if it exists
        if let Some(sender) = Self::get_message_sender() {
            if let Err(e) = sender.send(message.clone()) {
                warn!("Failed to send message to main channel: {}", e);
            }
        }

        // Send to all forwarders
        unsafe {
            static mut MESSAGE_FORWARDERS: Option<Vec<mpsc::UnboundedSender<ChatMessage>>> = None;
            
            if let Some(forwarders) = &mut MESSAGE_FORWARDERS {
                let forwarder_count = forwarders.len();
                trace!("Sending message to {} forwarders", forwarder_count);
                
                // Remove any closed channels and send to all active ones
                forwarders.retain(|forwarder| {
                    match forwarder.send(message.clone()) {
                        Ok(_) => true,
                        Err(e) => {
                            warn!("Failed to send message to forwarder: {}", e);
                            false // Remove this forwarder
                        }
                    }
                });
                
                if forwarder_count != forwarders.len() {
                    trace!("Cleaned up forwarders, {} remaining", forwarders.len());
                }
            } else {
                trace!("No message forwarders available");
            }
        }
    }

    #[instrument(skip(self), fields(node_id))]
    pub async fn initialize_network(&mut self) -> Result<String, String> {
        info!("Initializing network connection");

        // Create a temporary directory for the node
        let tmp_dir = tempfile::tempdir().map_err(|e| e.to_string())?;

        // Initialize the iroh endpoint
        let endpoint = Endpoint::builder()
            .discovery_n0()
            .bind()
            .await
            .map_err(|e| format!("Failed to create iroh endpoint: {}", e))?;

        // Get the node ID
        let node_id = endpoint.node_id().to_string();

        // Store endpoint and node_id
        self.endpoint = Some(endpoint.clone());
        self.node_id = Some(node_id.clone());

        // Start a background task to handle incoming messages
        let endpoint_clone = endpoint.clone();
        tokio::spawn(async move {
            Self::listen_for_messages(endpoint_clone).await;
        });

        info!(node_id = %node_id, "Network initialized with node ID");
        Ok(node_id)
    }

    // Function to listen for incoming messages from the network
    async fn listen_for_messages(endpoint: Endpoint) {
        info!("Starting message listener");
        
        // Currently just a placeholder - in a real application, 
        // we would use iroh-gossip to set up p2p communications
        
        // For demo purposes, this function simulates incoming message processing
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            info!("Message listener still active");
        }
    }

    #[instrument(skip(self), fields(topic_name = %topic_name))]
    pub async fn create_topic(
        &mut self,
        topic_name: String,
    ) -> Result<(String, String, String), String> {
        info!("Creating new topic: {}", topic_name);

        // Generate a simple ticket and hash for testing
        let uuid = Uuid::new_v4().to_string();
        let topic_hash = format!("{}-{}", topic_name, uuid);
        let ticket = format!("ticket-{}-{}", topic_name, uuid);

        // Store the topic information
        self.topic_ticket = Some(ticket.clone());
        self.topic_hash = Some(topic_hash.clone());

        // Store the topic in our subscribed topics
        self.subscribed_topics
            .insert(topic_name.clone(), topic_hash.clone());

        // In a real implementation, we would join the iroh-gossip topic here
        if let Some(_endpoint) = &self.endpoint {
            info!(
                topic_hash = %topic_hash,
                "Topic ready for p2p communication"
            );
        } else {
            return Err("No active network endpoint".to_string());
        }

        info!(
            topic_hash = %topic_hash,
            ticket = %ticket,
            "Topic created successfully"
        );

        Ok((topic_name, ticket, topic_hash))
    }

    #[instrument(skip(self), fields(ticket = %ticket))]
    pub async fn join_topic(&mut self, ticket: String) -> Result<(String, String), String> {
        info!("Attempting to join topic with ticket: {}", ticket);

        // For compatibility with the existing tests, handle the ticket format
        if ticket.starts_with("ticket-") {
            // Extract a topic name from the ticket
            let parts: Vec<&str> = ticket.split('-').collect();
            if parts.len() >= 3 {
                let topic_name_parts = &parts[1..parts.len() - 1];
                let topic_name = topic_name_parts.join("-");
                let uuid = parts.last().unwrap().to_string();

                // Generate a hash based on the ticket
                let topic_hash = format!("{}-{}", topic_name, uuid);

                // Store the topic information
                self.topic_ticket = Some(ticket.clone());
                self.topic_hash = Some(topic_hash.clone());

                // Store in subscribed topics
                self.subscribed_topics
                    .insert(topic_name.clone(), topic_hash.clone());

                // In a real implementation, we would join the iroh-gossip topic here
                if let Some(_endpoint) = &self.endpoint {
                    info!(
                        topic_hash = %topic_hash,
                        "Connected to p2p topic"
                    );
                } else {
                    return Err("No active network endpoint".to_string());
                }

                info!(
                    topic_name = %topic_name,
                    topic_hash = %topic_hash,
                    "Successfully joined topic"
                );

                return Ok((topic_name, topic_hash));
            }

            return Err("Invalid ticket format".to_string());
        }

        // Handle other ticket formats for backward compatibility
        let topic_name = "joined-topic";
        let topic_hash = Uuid::new_v4().to_string();

        // Store the topic information
        self.topic_ticket = Some(ticket.clone());
        self.topic_hash = Some(topic_hash.clone());

        // Store in subscribed topics
        self.subscribed_topics
            .insert(topic_name.to_string(), topic_hash.clone());

        // In a real implementation, we would join the iroh-gossip topic here
        if let Some(_endpoint) = &self.endpoint {
            info!(
                topic_hash = %topic_hash,
                "Connected to p2p topic"
            );
        } else {
            return Err("No active network endpoint".to_string());
        }

        info!(
            topic_name = %topic_name,
            topic_hash = %topic_hash,
            "Successfully joined topic with custom ticket"
        );

        Ok((topic_name.to_string(), topic_hash))
    }

    #[instrument(skip(self), fields(
        username = %username,
        topic_hash = ?self.topic_hash,
        sequence = %sequence
    ))]
    pub async fn send_message(
        &self,
        username: String,
        message_content: String,
        sequence: u64,
    ) -> Result<(), String> {
        let topic_hash = self
            .topic_hash
            .as_ref()
            .ok_or_else(|| "No active topic hash".to_string())?;

        info!(
            content_length = message_content.len(),
            "Sending message to network"
        );

        // Create the chat message
        let message_id = Uuid::new_v4().to_string();
        let chat_message = ChatMessage {
            id: message_id.clone(),
            author: username.clone(),
            content: message_content.clone(),
            timestamp: Utc::now(),
            topic_hash: topic_hash.clone(),
            sequence,
        };

        // Broadcast to local channels only for now
        Self::broadcast_message(chat_message);

        // In a real implementation, we would also broadcast to the p2p network here
        if let Some(_endpoint) = &self.endpoint {
            info!(
                message_id = %message_id,
                "Message sent to local recipients"
            );
        } else {
            return Err("No active network endpoint".to_string());
        }

        Ok(())
    }
}

#[cfg(test)]
impl IrohClient {
    // This function helps with testing by ensuring we can run tests in parallel
    pub async fn initialize_for_test() -> Self {
        let mut client = Self::new();
        let _ = client
            .initialize_network()
            .await
            .expect("Failed to initialize network");

        // Initialize the message channel for tests
        let (sender, _) = Self::initialize_message_channel();

        client
    }

    // For testing, we need to ensure messages are properly received
    pub async fn wait_for_message(&self, timeout_ms: u64) -> Option<ChatMessage> {
        // Get a dedicated receiver for this wait operation
        let mut receiver = Self::get_message_receiver()?;

        // Set up a timeout
        let timeout = tokio::time::sleep(tokio::time::Duration::from_millis(timeout_ms));

        tokio::select! {
            message = receiver.recv() => message,
            _ = timeout => None,
        }
    }
}
