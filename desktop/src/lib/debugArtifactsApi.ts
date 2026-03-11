import { invoke } from '@tauri-apps/api/core';
import type { CommandResponse } from './tauri';
import type { DebugArtifactContent, DebugArtifactDescriptor } from '../types/debugMode';

export async function listDebugArtifacts(sessionId: string): Promise<CommandResponse<DebugArtifactDescriptor[]>> {
  try {
    return await invoke<CommandResponse<DebugArtifactDescriptor[]>>('list_debug_artifacts', {
      sessionId,
    });
  } catch (error) {
    return {
      success: false,
      data: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

export async function loadDebugArtifact(
  sessionId: string,
  artifactPath: string,
): Promise<CommandResponse<DebugArtifactContent>> {
  try {
    return await invoke<CommandResponse<DebugArtifactContent>>('load_debug_artifact', {
      sessionId,
      artifactPath,
    });
  } catch (error) {
    return {
      success: false,
      data: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

export async function writeDebugArtifact(
  sessionId: string,
  fileName: string,
  content: string,
): Promise<CommandResponse<DebugArtifactDescriptor>> {
  try {
    return await invoke<CommandResponse<DebugArtifactDescriptor>>('write_debug_artifact', {
      sessionId,
      request: {
        sessionId,
        fileName,
        content,
      },
    });
  } catch (error) {
    return {
      success: false,
      data: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}
