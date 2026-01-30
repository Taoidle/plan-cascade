/**
 * Project and Session Types
 *
 * TypeScript interfaces matching the Rust models in desktop/src-tauri/src/models/
 */

/** Project model - represents a Claude Code project */
export interface Project {
  id: string;
  name: string;
  path: string;
  last_activity: string;
  session_count: number;
  message_count: number;
}

/** Session model - represents a session within a project */
export interface Session {
  id: string;
  project_id: string;
  file_path: string;
  created_at: string;
  last_activity: string;
  message_count: number;
  first_message_preview: string | null;
}

/** Detailed session information */
export interface SessionDetails {
  session: Session;
  messages: MessageSummary[];
  checkpoint_count: number;
}

/** Summary of a message in a session */
export interface MessageSummary {
  message_type: string;
  content_preview: string;
  timestamp: string | null;
}

/** Result of preparing to resume a session */
export interface ResumeResult {
  success: boolean;
  session_path: string;
  resume_command: string;
  error_message: string | null;
}

/** Sort options for projects */
export type ProjectSortBy = 'recent_activity' | 'name' | 'session_count';

/** Generic command response from Tauri */
export interface CommandResponse<T> {
  success: boolean;
  data: T | null;
  error: string | null;
}
