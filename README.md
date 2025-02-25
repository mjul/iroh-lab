# Iroh Chat

A simple peer-to-peer chat application built with Iroh and Iced UI.

## Overview

Iroh Chat is a decentralized chat application that leverages Iroh's peer-to-peer networking capabilities to create secure, direct communication channels between users. The application features a clean, intuitive interface built with Iced, a cross-platform GUI library for Rust.

## Features

- **User-friendly Interface**: Simple and intuitive UI for seamless chatting experience
- **Create Chat Topics**: Start new conversations and generate shareable tickets
- **Join Existing Topics**: Connect to ongoing conversations using tickets
- **Real-time Messaging**: Send and receive messages instantly
- **Decentralized Architecture**: No central servers, direct peer-to-peer communication

## Technical Stack

- **Iroh**: Provides the peer-to-peer networking and data synchronization capabilities
- **Iced**: Powers the cross-platform GUI
- **Tokio**: Handles asynchronous operations
- **Serde**: Manages serialization and deserialization of messages

## Getting Started

### Prerequisites

- Rust and Cargo installed on your system

### Installation

1. Clone the repository:
   ```
   git clone https://github.com/yourusername/iroh-chat.git
   cd iroh-chat
   ```

2. Build the application:
   ```
   cargo build --release
   ```

3. Run the application:
   ```
   cargo run --release
   ```

### Usage

1. **Start the application**: Launch Iroh Chat
2. **Enter a username**: Identify yourself in the chat
3. **Create a new topic**: Start a new conversation and share the generated ticket with others
4. **Join a topic**: Paste a ticket to join an existing conversation
5. **Chat**: Exchange messages in real-time with other participants

## Implementation Details

The application is structured around the Iced application framework and uses Iroh's client API for peer-to-peer communication. Key components include:

- **UI State Management**: Handles different screens (welcome, menu, chat room)
- **Iroh Integration**: Manages topic creation, joining, and message exchange
- **Message Handling**: Processes incoming and outgoing messages

## Future Improvements

- End-to-end encryption for messages
- File sharing capabilities
- Offline message queuing
- User presence indicators
- Message history persistence

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Acknowledgments

- [Iroh](https://iroh.computer/) for the peer-to-peer networking library
- [Iced](https://github.com/iced-rs/iced) for the GUI framework 