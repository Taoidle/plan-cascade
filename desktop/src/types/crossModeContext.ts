/**
 * Cross-Mode Context Types
 *
 * Types for sharing conversation context between Chat and Task modes.
 * ConversationTurn is a simple user/assistant pair used for IPC serialization
 * when passing conversation history to the Rust backend.
 */

/** A conversation turn passed to the Rust backend via IPC */
export interface CrossModeConversationTurn {
  user: string;
  assistant: string;
}
