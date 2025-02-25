use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
}

impl IrohClient {
    pub fn new() -> Self {
        trace!("Creating new IrohClient instance");
        Self {
            node_id: None,
            topic_ticket: None,
            topic_hash: None,
            subscribed_topics: HashMap::new(),
        }
    }

    pub fn initialize_message_channel() -> (
        mpsc::UnboundedSender<ChatMessage>,
        mpsc::UnboundedReceiver<ChatMessage>,
    ) {
        trace!("Initializing message channel");
        let (sender, receiver) = mpsc::unbounded_channel();
        unsafe {
            MESSAGE_SENDER = Some(sender.clone());
            // Don't move the receiver, just store a clone of it
            if MESSAGE_RECEIVER.is_none() {
                MESSAGE_RECEIVER = Some(receiver);
                trace!("Message receiver initialized");
                return (sender, mpsc::unbounded_channel().1); // Return a dummy receiver
            }
        }
        trace!("Using existing message receiver");
        (sender, mpsc::unbounded_channel().1) // Return a dummy receiver
    }

    pub fn get_message_sender() -> Option<mpsc::UnboundedSender<ChatMessage>> {
        trace!("Getting message sender");
        unsafe { MESSAGE_SENDER.clone() }
    }

    pub fn get_message_receiver() -> Option<mpsc::UnboundedReceiver<ChatMessage>> {
        trace!("Taking message receiver");
        unsafe { MESSAGE_RECEIVER.take() }
    }

    #[instrument(skip(self), fields(node_id))]
    pub async fn initialize_network(&mut self) -> Result<String, String> {
        info!("Initializing network connection");
        // Generate a random node ID
        let node_id = Uuid::new_v4().to_string();
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
        // Create a topic hash from the topic name
        let topic_hash = format!("{}-{}", topic_name, Uuid::new_v4());

        // Generate a ticket for sharing
        let ticket = format!("ticket-{}-{}", topic_name, topic_hash);

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
        // Parse the ticket to extract the topic name and hash
        // Expected format: ticket-{topic_name}-{hash}
        if ticket.starts_with("ticket-") {
            let parts: Vec<&str> = ticket.split('-').collect();
            if parts.len() >= 3 {
                // Extract the topic name
                let topic_name_parts = &parts[1..parts.len() - 1];
                let topic_name = topic_name_parts.join("-");

                // Extract the hash
                let hash = parts[parts.len() - 1].to_string();

                self.topic_hash = Some(hash.clone());

                // Store the topic in our subscribed topics
                self.subscribed_topics
                    .insert(topic_name.clone(), hash.clone());

                info!(
                    topic_name = %topic_name,
                    topic_hash = %hash,
                    "Successfully joined topic"
                );

                return Ok((topic_name, hash));
            }
        }

        warn!("Failed to join topic: Invalid ticket format");
        Err("Invalid ticket format".to_string())
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
        if let Some(topic_hash) = &self.topic_hash {
            info!(
                content_length = message_content.len(),
                "Sending message to network"
            );

            // Create the chat message - we don't use this directly but it's useful for debugging
            let message_id = Uuid::new_v4().to_string();
            let _chat_message = ChatMessage {
                id: message_id.clone(),
                author: username.clone(),
                content: message_content.clone(),
                timestamp: Utc::now(),
                topic_hash: topic_hash.clone(),
                sequence,
            };

            trace!(
                message_id = %message_id,
                "Message created and ready to send"
            );

            // In a real implementation, we would publish the message to the network
            // For now, we'll simulate receiving the message from another client
            let sender_clone = Self::get_message_sender();

            // Simulate network delay
            trace!("Simulating network delay");
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

            // Simulate receiving the message from another user
            if let Some(sender) = sender_clone {
                // Create a simulated message from another user
                let simulated_user_id = Uuid::new_v4().to_string()[..8].to_string();
                let simulated_message_id = Uuid::new_v4().to_string();

                trace!(
                    simulated_user = %simulated_user_id,
                    simulated_message_id = %simulated_message_id,
                    "Creating simulated response message"
                );

                let simulated_message = ChatMessage {
                    id: simulated_message_id,
                    author: format!("User-{}", simulated_user_id),
                    content: format!("Reply to: {}", message_content),
                    timestamp: Utc::now(),
                    topic_hash: topic_hash.clone(),
                    sequence: sequence + 1,
                };

                // Send the simulated message
                trace!("Sending simulated message to channel");
                if let Err(e) = sender.send(simulated_message) {
                    warn!(error = %e, "Failed to send simulated message");
                } else {
                    info!("Simulated message sent successfully");
                }
            } else {
                warn!("No message sender available");
            }

            info!("Message sending process completed");
            Ok(())
        } else {
            warn!("Cannot send message: No active topic");
            Err("No active topic".to_string())
        }
    }
}
