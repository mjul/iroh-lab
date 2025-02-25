use iced::{
    alignment, executor, Application, Command, Element, Length, Settings, Subscription,
    Theme, widget::{button, column, container, row, scrollable, text, text_input}, Alignment,
    clipboard, time,
};
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

// Channel for receiving messages from the network
static mut MESSAGE_SENDER: Option<mpsc::UnboundedSender<ChatMessage>> = None;
static mut MESSAGE_RECEIVER: Option<mpsc::UnboundedReceiver<ChatMessage>> = None;

fn main() -> iced::Result {
    // Initialize the message channel
    let (sender, receiver) = mpsc::unbounded_channel();
    unsafe {
        MESSAGE_SENDER = Some(sender);
        MESSAGE_RECEIVER = Some(receiver);
    }
    
    IrohChat::run(Settings::default())
}

// Message structure for chat
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChatMessage {
    id: String,
    author: String,
    content: String,
    timestamp: DateTime<Utc>,
    topic_hash: String,
    sequence: u64,
}

// Application state
struct IrohChat {
    // UI state
    input_state: InputState,
    
    // Chat state
    current_topic: Option<String>,
    messages: Vec<ChatMessage>,
    processed_message_ids: HashSet<String>,
    sequence_counter: u64,
    
    // Network state
    node_id: Option<String>,
    
    // Topic management
    topic_ticket: Option<String>,
    topic_hash: Option<String>,
    subscribed_topics: HashMap<String, String>,
    
    // Error message
    error: Option<String>,
}

// Input state for different screens
#[derive(Clone)]
enum InputState {
    Welcome {
        username: String,
    },
    MainMenu {
        username: String,
    },
    CreateTopic {
        username: String,
        topic_name: String,
    },
    JoinTopic {
        username: String,
        ticket: String,
    },
    TopicCreated {
        username: String,
        topic_name: String,
        ticket: String,
    },
    ChatRoom {
        username: String,
        message: String,
    },
}

// Messages for the Iced application
#[derive(Debug, Clone)]
enum Message {
    // Input events
    UsernameChanged(String),
    TopicNameChanged(String),
    TicketChanged(String),
    MessageChanged(String),
    
    // Button events
    SubmitUsername,
    CreateTopicSelected,
    JoinTopicSelected,
    BackToMenu,
    SubmitCreateTopic,
    SubmitJoinTopic,
    EnterChatRoom,
    SendMessage,
    
    // Clipboard
    CopyTicket,
    
    // Network events
    NetworkInitialized(Result<String, String>),
    TopicCreated(Result<(String, String, String), String>),
    TopicJoined(Result<(String, String), String>),
    MessageReceived(ChatMessage),
    MessageSent,
    
    // Polling for messages
    Tick,
    CheckForMessages,
}

impl Application for IrohChat {
    type Executor = executor::Default;
    type Message = Message;
    type Theme = Theme;
    type Flags = ();

    fn new(_flags: ()) -> (Self, Command<Message>) {
        let app = Self {
            input_state: InputState::Welcome {
                username: String::new(),
            },
            current_topic: None,
            messages: Vec::new(),
            processed_message_ids: HashSet::new(),
            sequence_counter: 0,
            node_id: None,
            topic_ticket: None,
            topic_hash: None,
            subscribed_topics: HashMap::new(),
            error: None,
        };

        // Initialize network
        let command = Command::perform(
            async {
                // Generate a random node ID
                let node_id = Uuid::new_v4().to_string();
                Ok(node_id)
            },
            Message::NetworkInitialized,
        );

        (app, command)
    }

    fn title(&self) -> String {
        match &self.current_topic {
            Some(topic) => format!("Chat - {}", topic),
            None => "Chat Application".to_string(),
        }
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::UsernameChanged(username) => {
                match &mut self.input_state {
                    InputState::Welcome { username: u } => *u = username,
                    InputState::MainMenu { username: u } => *u = username,
                    InputState::CreateTopic { username: u, .. } => *u = username,
                    InputState::JoinTopic { username: u, .. } => *u = username,
                    InputState::TopicCreated { username: u, .. } => *u = username,
                    InputState::ChatRoom { username: u, .. } => *u = username,
                }
                Command::none()
            }
            
            Message::TopicNameChanged(topic_name) => {
                if let InputState::CreateTopic { topic_name: t, .. } = &mut self.input_state {
                    *t = topic_name;
                }
                Command::none()
            }
            
            Message::TicketChanged(ticket) => {
                if let InputState::JoinTopic { ticket: t, .. } = &mut self.input_state {
                    *t = ticket;
                }
                Command::none()
            }
            
            Message::MessageChanged(message) => {
                if let InputState::ChatRoom { message: m, .. } = &mut self.input_state {
                    *m = message;
                }
                Command::none()
            }
            
            Message::SubmitUsername => {
                if let InputState::Welcome { username } = &self.input_state {
                    if !username.trim().is_empty() {
                        self.input_state = InputState::MainMenu {
                            username: username.clone(),
                        };
                    }
                }
                Command::none()
            }
            
            Message::CreateTopicSelected => {
                if let InputState::MainMenu { username } = &self.input_state {
                    self.input_state = InputState::CreateTopic {
                        username: username.clone(),
                        topic_name: String::new(),
                    };
                }
                Command::none()
            }
            
            Message::JoinTopicSelected => {
                if let InputState::MainMenu { username } = &self.input_state {
                    self.input_state = InputState::JoinTopic {
                        username: username.clone(),
                        ticket: String::new(),
                    };
                }
                Command::none()
            }
            
            Message::BackToMenu => {
                if let Some(username) = self.get_username() {
                    self.input_state = InputState::MainMenu {
                        username,
                    };
                    self.current_topic = None;
                    self.messages.clear();
                }
                Command::none()
            }
            
            Message::SubmitCreateTopic => {
                if let InputState::CreateTopic { username, topic_name } = &self.input_state.clone() {
                    if !topic_name.trim().is_empty() {
                        let username = username.clone();
                        let topic_name = topic_name.clone();
                        
                        return Command::perform(
                            async move {
                                // Create a topic hash from the topic name
                                let topic_hash = format!("{}-{}", topic_name, Uuid::new_v4());
                                
                                // Generate a ticket for sharing
                                let ticket = format!("ticket-{}-{}", topic_name, topic_hash);
                                Ok((topic_name, ticket, topic_hash))
                            },
                            |result| {
                                match result {
                                    Ok((topic_name, ticket, hash)) => Message::TopicCreated(Ok((topic_name, ticket, hash))),
                                    Err(e) => Message::TopicCreated(Err(e)),
                                }
                            },
                        );
                    }
                }
                Command::none()
            }
            
            Message::SubmitJoinTopic => {
                if let InputState::JoinTopic { username, ticket } = &self.input_state.clone() {
                    if !ticket.trim().is_empty() {
                        let _username = username.clone();
                        let ticket = ticket.clone();
                        
                        return Command::perform(
                            async move {
                                // Parse the ticket to extract the topic name and hash
                                // Expected format: ticket-{topic_name}-{hash}
                                if ticket.starts_with("ticket-") {
                                    let parts: Vec<&str> = ticket.split('-').collect();
                                    if parts.len() >= 3 {
                                        // Extract the topic name
                                        let topic_name_parts = &parts[1..parts.len()-1];
                                        let topic_name = topic_name_parts.join("-");
                                        
                                        // Extract the hash
                                        let hash = parts[parts.len()-1].to_string();
                                        
                                        return Ok((topic_name, hash));
                                    }
                                }
                                
                                Err("Invalid ticket format".to_string())
                            },
                            |result| {
                                match result {
                                    Ok((topic_name, hash)) => Message::TopicJoined(Ok((topic_name, hash))),
                                    Err(e) => Message::TopicJoined(Err(e)),
                                }
                            },
                        );
                    }
                }
                Command::none()
            }
            
            Message::CopyTicket => {
                if let InputState::TopicCreated { ticket, .. } = &self.input_state {
                    return Command::batch(vec![
                        clipboard::write(ticket.clone()),
                    ]);
                }
                Command::none()
            }
            
            Message::EnterChatRoom => {
                if let InputState::TopicCreated { username, topic_name, .. } = &self.input_state.clone() {
                    self.input_state = InputState::ChatRoom {
                        username: username.clone(),
                        message: String::new(),
                    };
                    self.current_topic = Some(topic_name.clone());
                }
                Command::none()
            }
            
            Message::SendMessage => {
                if let InputState::ChatRoom { username, message } = &self.input_state.clone() {
                    if !message.trim().is_empty() && self.current_topic.is_some() && self.topic_hash.is_some() {
                        let username = username.clone();
                        let message_content = message.clone();
                        let topic_hash = self.topic_hash.clone().unwrap();
                        let sequence = self.sequence_counter;
                        
                        // Increment sequence counter
                        self.sequence_counter += 1;
                        
                        // Clear the message input
                        if let InputState::ChatRoom { message: m, .. } = &mut self.input_state {
                            *m = String::new();
                        }
                        
                        // Create the chat message
                        let chat_message = ChatMessage {
                            id: Uuid::new_v4().to_string(),
                            author: username.clone(),
                            content: message_content.clone(),
                            timestamp: Utc::now(),
                            topic_hash: topic_hash.clone(),
                            sequence,
                        };
                        
                        // Add message to local state
                        self.messages.push(chat_message.clone());
                        self.processed_message_ids.insert(chat_message.id.clone());
                        
                        // In a real implementation, we would publish the message to the network
                        // For now, we'll simulate receiving the message from another client
                        let sender_clone = unsafe { MESSAGE_SENDER.clone() };
                        
                        return Command::perform(
                            async move {
                                // Simulate network delay
                                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                                
                                // Simulate receiving the message from another client
                                if let Some(sender) = sender_clone {
                                    // Create a simulated message from another user
                                    let simulated_message = ChatMessage {
                                        id: Uuid::new_v4().to_string(),
                                        author: format!("User-{}", Uuid::new_v4().to_string()[..8].to_string()),
                                        content: format!("Reply to: {}", message_content),
                                        timestamp: Utc::now(),
                                        topic_hash: topic_hash.clone(),
                                        sequence: sequence + 1,
                                    };
                                    
                                    // Send the simulated message
                                    let _ = sender.send(simulated_message);
                                }
                                
                                Ok(()) as Result<(), String>
                            },
                            |result: Result<(), String>| {
                                match result {
                                    Ok(_) => Message::MessageSent,
                                    Err(e) => {
                                        println!("Error sending message: {}", e);
                                        Message::MessageSent
                                    }
                                }
                            },
                        );
                    }
                }
                Command::none()
            }
            
            Message::NetworkInitialized(result) => {
                match result {
                    Ok(node_id) => {
                        self.node_id = Some(node_id);
                    }
                    Err(error) => {
                        self.error = Some(error);
                    }
                }
                Command::none()
            }
            
            Message::TopicCreated(result) => {
                match result {
                    Ok((topic, ticket, hash)) => {
                        self.current_topic = Some(topic.clone());
                        self.topic_ticket = Some(ticket.clone());
                        self.topic_hash = Some(hash.clone());
                        
                        // Store the topic in our subscribed topics
                        self.subscribed_topics.insert(topic.clone(), hash.clone());
                        
                        if let Some(username) = self.get_username() {
                            self.input_state = InputState::TopicCreated {
                                username,
                                topic_name: topic,
                                ticket,
                            };
                        }
                    }
                    Err(error) => {
                        self.error = Some(error);
                    }
                }
                Command::none()
            }
            
            Message::TopicJoined(result) => {
                match result {
                    Ok((topic, hash)) => {
                        self.current_topic = Some(topic.clone());
                        self.topic_hash = Some(hash.clone());
                        
                        // Store the topic in our subscribed topics
                        self.subscribed_topics.insert(topic.clone(), hash.clone());
                        
                        if let Some(username) = self.get_username() {
                            self.input_state = InputState::ChatRoom {
                                username,
                                message: String::new(),
                            };
                        }
                    }
                    Err(error) => {
                        self.error = Some(error);
                    }
                }
                Command::none()
            }
            
            Message::MessageReceived(message) => {
                // Only add the message if it's not already in our list
                if !self.processed_message_ids.contains(&message.id) {
                    self.messages.push(message.clone());
                    self.processed_message_ids.insert(message.id);
                }
                Command::none()
            }
            
            Message::MessageSent => {
                // Message was sent successfully
                Command::none()
            }
            
            Message::Tick => {
                // Check if there are any new messages in the channel
                unsafe {
                    if let Some(ref mut receiver) = MESSAGE_RECEIVER {
                        // Try to receive all pending messages
                        let mut commands = Vec::new();
                        
                        while let Ok(message) = receiver.try_recv() {
                            commands.push(Command::perform(
                                async move { message },
                                Message::MessageReceived,
                            ));
                        }
                        
                        if !commands.is_empty() {
                            return Command::batch(commands);
                        }
                    }
                }
                
                Command::none()
            }
            
            Message::CheckForMessages => {
                // This is now handled by the message listener
                Command::none()
            }
        }
    }

    fn view(&self) -> Element<Message> {
        match &self.input_state {
            InputState::Welcome { username } => {
                let title = text("Welcome to Chat")
                    .size(30)
                    .width(Length::Fill)
                    .horizontal_alignment(alignment::Horizontal::Center);
                
                let username_input = text_input("Enter your username", username)
                    .on_input(Message::UsernameChanged)
                    .padding(10);
                
                let submit_button = button("Continue")
                    .on_press(Message::SubmitUsername)
                    .padding(10);
                
                let content = column![
                    title,
                    username_input,
                    submit_button,
                ]
                .spacing(20)
                .padding(20)
                .width(Length::Fill)
                .height(Length::Fill)
                .align_items(Alignment::Center);
                
                container(content)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .center_x()
                    .center_y()
                    .into()
            }
            
            InputState::MainMenu { username } => {
                let title = text(format!("Hello, {}! What would you like to do?", username))
                    .size(24)
                    .width(Length::Fill)
                    .horizontal_alignment(alignment::Horizontal::Center);
                
                let create_button = button("Create a new topic")
                    .on_press(Message::CreateTopicSelected)
                    .padding(10)
                    .width(Length::Fill);
                
                let join_button = button("Join an existing topic")
                    .on_press(Message::JoinTopicSelected)
                    .padding(10)
                    .width(Length::Fill);
                
                let content = column![
                    title,
                    create_button,
                    join_button,
                ]
                .spacing(20)
                .padding(20)
                .width(Length::Fill)
                .max_width(400)
                .align_items(Alignment::Center);
                
                container(content)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .center_x()
                    .center_y()
                    .into()
            }
            
            InputState::CreateTopic { username: _, topic_name } => {
                let title = text("Create a New Topic")
                    .size(24)
                    .width(Length::Fill)
                    .horizontal_alignment(alignment::Horizontal::Center);
                
                let topic_input = text_input("Enter topic name", topic_name)
                    .on_input(Message::TopicNameChanged)
                    .padding(10);
                
                let button_row = row![
                    button("Back")
                        .on_press(Message::BackToMenu)
                        .padding(10),
                    button("Create")
                        .on_press(Message::SubmitCreateTopic)
                        .padding(10),
                ]
                .spacing(10)
                .width(Length::Fill);
                
                let content = column![
                    title,
                    topic_input,
                    button_row,
                ]
                .spacing(20)
                .padding(20)
                .width(Length::Fill)
                .max_width(400)
                .align_items(Alignment::Center);
                
                container(content)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .center_x()
                    .center_y()
                    .into()
            }
            
            InputState::JoinTopic { username: _, ticket } => {
                let title = text("Join an Existing Topic")
                    .size(24)
                    .width(Length::Fill)
                    .horizontal_alignment(alignment::Horizontal::Center);
                
                let ticket_input = text_input("Paste the ticket", ticket)
                    .on_input(Message::TicketChanged)
                    .padding(10);
                
                let button_row = row![
                    button("Back")
                        .on_press(Message::BackToMenu)
                        .padding(10),
                    button("Join")
                        .on_press(Message::SubmitJoinTopic)
                        .padding(10),
                ]
                .spacing(10)
                .width(Length::Fill);
                
                let content = column![
                    title,
                    ticket_input,
                    button_row,
                ]
                .spacing(20)
                .padding(20)
                .width(Length::Fill)
                .max_width(400)
                .align_items(Alignment::Center);
                
                container(content)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .center_x()
                    .center_y()
                    .into()
            }
            
            InputState::TopicCreated { username: _, topic_name, ticket } => {
                let title = text(format!("Topic '{}' Created Successfully!", topic_name))
                    .size(24)
                    .width(Length::Fill)
                    .horizontal_alignment(alignment::Horizontal::Center);
                
                let ticket_text = text("Share this ticket with others to let them join:")
                    .size(16)
                    .width(Length::Fill)
                    .horizontal_alignment(alignment::Horizontal::Center);
                
                let ticket_row = row![
                    text(ticket)
                        .size(18)
                        .width(Length::Fill)
                        .horizontal_alignment(alignment::Horizontal::Center),
                    button("Copy")
                        .on_press(Message::CopyTicket)
                        .padding(5),
                ]
                .spacing(10)
                .width(Length::Fill)
                .align_items(Alignment::Center);
                
                let button_row = row![
                    button("Back to Menu")
                        .on_press(Message::BackToMenu)
                        .padding(10),
                    button("Enter Chat Room")
                        .on_press(Message::EnterChatRoom)
                        .padding(10),
                ]
                .spacing(10)
                .width(Length::Fill);
                
                let content = column![
                    title,
                    ticket_text,
                    ticket_row,
                    button_row,
                ]
                .spacing(20)
                .padding(20)
                .width(Length::Fill)
                .max_width(600)
                .align_items(Alignment::Center);
                
                container(content)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .center_x()
                    .center_y()
                    .into()
            }
            
            InputState::ChatRoom { username: _, message } => {
                let title = text(format!("Topic: {}", self.current_topic.as_ref().unwrap_or(&"Unknown".to_string())))
                    .size(24)
                    .width(Length::Fill)
                    .horizontal_alignment(alignment::Horizontal::Center);
                
                // Create the message list
                let messages = self.messages.iter().fold(
                    column![].spacing(10).width(Length::Fill),
                    |column, msg| {
                        let message_text = format!("{}: {}", msg.author, msg.content);
                        let timestamp = msg.timestamp.format("%H:%M:%S").to_string();
                        
                        column.push(
                            row![
                                text(message_text).width(Length::Fill),
                                text(timestamp).size(12),
                            ]
                            .spacing(10)
                            .width(Length::Fill)
                        )
                    }
                );
                
                let messages_scrollable = scrollable(messages)
                    .height(Length::Fill)
                    .width(Length::Fill);
                
                let input_row = row![
                    text_input("Type a message", message)
                        .on_input(Message::MessageChanged)
                        .padding(10)
                        .width(Length::Fill),
                    button("Send")
                        .on_press(Message::SendMessage)
                        .padding(10),
                ]
                .spacing(10)
                .width(Length::Fill);
                
                let content = column![
                    row![
                        title,
                        button("Leave")
                            .on_press(Message::BackToMenu)
                            .padding(5),
                    ].spacing(10).width(Length::Fill),
                    messages_scrollable,
                    input_row,
                ]
                .spacing(20)
                .padding(20)
                .width(Length::Fill)
                .height(Length::Fill);
                
                container(content)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .into()
            }
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        // Only subscribe to events when in a chat room
        if let InputState::ChatRoom { .. } = self.input_state {
            // Create a subscription that ticks every second to check for new messages
            Subscription::batch(vec![
                time::every(std::time::Duration::from_secs(1))
                    .map(|_| Message::Tick),
            ])
        } else {
            Subscription::none()
        }
    }
}

impl IrohChat {
    fn get_username(&self) -> Option<String> {
        match &self.input_state {
            InputState::Welcome { username } => Some(username.clone()),
            InputState::MainMenu { username } => Some(username.clone()),
            InputState::CreateTopic { username, .. } => Some(username.clone()),
            InputState::JoinTopic { username, .. } => Some(username.clone()),
            InputState::TopicCreated { username, .. } => Some(username.clone()),
            InputState::ChatRoom { username, .. } => Some(username.clone()),
        }
    }
}
