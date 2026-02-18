//! Remote Session Control
//!
//! Enables remote interaction with the desktop client through messaging platforms.
//! Currently supports Telegram Bot as the primary adapter.
//!
//! ## Architecture
//!
//! ```text
//! Remote Platform → RemoteAdapter → mpsc → RemoteGatewayService
//!                                            ↓
//!                                   CommandRouter.parse()
//!                                            ↓
//!                                   SessionBridge (create/send/cancel)
//!                                            ↓
//!                                   ResponseMapper → RemoteAdapter.send_message()
//! ```

pub mod adapters;
pub mod command_router;
pub mod gateway;
pub mod response_mapper;
pub mod session_bridge;
pub mod types;

pub use types::*;
