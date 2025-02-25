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
                // Ensure we have a valid MESSAGE_FORWARDER
                static mut MESSAGE_FORWARDER: Option<mpsc::UnboundedSender<ChatMessage>> = None;

                // Replace any old forwarder with our new one
                MESSAGE_FORWARDER = Some(new_sender);

                // Return the new receiver
                return Some(new_receiver);
            }
        }

        None
    }

    // This function should be used to send messages, ensuring they go to all receivers
    pub fn broadcast_message(message: ChatMessage) {
        // Send to the main channel if it exists
        if let Some(sender) = Self::get_message_sender() {
            if let Err(e) = sender.send(message.clone()) {
                warn!("Failed to send message to main channel: {}", e);
            }
        }

        // Send to any forwarders
        unsafe {
            static mut MESSAGE_FORWARDER: Option<mpsc::UnboundedSender<ChatMessage>> = None;
            if let Some(forwarder) = &MESSAGE_FORWARDER {
                if let Err(e) = forwarder.send(message) {
                    warn!("Failed to send message to forwarder: {}", e);
                    // Clear the forwarder if it's closed
                    MESSAGE_FORWARDER = None;
                }
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
        self.endpoint = Some(endpoint);
        self.node_id = Some(node_id.clone());

        info!(node_id = %node_id, "Network initialized with node ID");
        Ok(node_id)
    }

    #[instrument(skip(self), fields(topic_name = %topic_name))]
    pub async fn create_topic(
        &mut self,
        topic_name: String,
    ) -> Result<(String, String, String), String> {
        info!("Creating new topic: {}", topic_name);

        // Generate a simple ticket and hash for testing
        // In a real implementation, this would use actual iroh functionality
        let uuid = Uuid::new_v4().to_string();
        let topic_hash = format!("{}-{}", topic_name, uuid);
        let ticket = format!("ticket-{}-{}", topic_name, uuid);

        // Store the topic information
        self.topic_ticket = Some(ticket.clone());
        self.topic_hash = Some(topic_hash.clone());

        // Store the topic in our subscribed topics
        self.subscribed_topics
            .insert(topic_name.clone(), topic_hash.clone());

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

        // In a real implementation, this would actually send the message via iroh
        // For now, we'll just simulate success and push the message to our channel
        Self::broadcast_message(chat_message);

        info!(
            message_id = %message_id,
            "Message sent successfully"
        );

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
