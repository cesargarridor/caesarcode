import { Channel, invoke } from '@tauri-apps/api/core';
import type { TerminalEvent } from './types';

export type AgentId = 'claude' | 'codex' | 'gemini';

export interface AgentInfo {
  id: AgentId;
  bin: string;
  found: boolean;
  path: string | null;
  installCmd: string;
}

export const agentsDetect = () => invoke<AgentInfo[]>('agents_detect');

/** Abre el agente en un PTY (argv vacío, cwd = raíz primaria). Devuelve el id de terminal. */
export function agentOpen(
  agent: AgentId,
  cols: number,
  rows: number,
  handler: (ev: TerminalEvent) => void,
): Promise<string> {
  const onEvent = new Channel<TerminalEvent>();
  onEvent.onmessage = handler;
  return invoke<string>('agent_open', { agent, cols, rows, onEvent });
}
