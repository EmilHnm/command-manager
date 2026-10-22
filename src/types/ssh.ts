/**
 * SSH Types for Command Manager (SCR-07, MOD-17, MOD-18, MOD-19)
 * Maps directly to Rust backend types in src-tauri/src/ssh and src-tauri/src/db/repos/ssh_hosts.rs
 */

export type SshAuthMethod = 'auto' | 'agent' | 'key' | 'password';
export type SshSecretStore = 'none' | 'keyring' | 'sqlite';
export type SshKeySource = 'file' | 'content';

export interface SshHost {
  id: string;
  name: string;
  host: string;
  port: number;
  username: string;
  auth_method: SshAuthMethod;
  key_path?: string | null;
  key_source: SshKeySource;
  /** Keychain identity: supplies username, password and maybe a key. */
  identity_id?: string | null;
  /** Keychain key; wins over the identity's key and over key_path. */
  key_id?: string | null;
  jump_host_id?: string | null;
  use_system_ssh: boolean;
  agent_forwarding: boolean;
  default_dir?: string | null;
  secret_store: SshSecretStore;
  color?: string | null;
  tags: string[];
  sort_order: number;
  created_at: string;
  last_connected_at?: string | null;
}

export interface SshHostView extends SshHost {
  open_channels: number;
}

/** Same cap as ssh_hosts::MAX_JUMP_HOPS. */
export const MAX_JUMP_HOPS = 5;

/**
 * Jump hosts crossed to reach a host whose jump host is `firstId`, in the
 * order they are crossed (outermost first). Stops at a loop or a missing host.
 */
export function jumpChain<T extends Pick<SshHost, 'id' | 'jump_host_id'>>(
  hosts: T[],
  firstId: string | null | undefined
): T[] {
  const chain: T[] = [];
  let next = firstId;
  while (next && chain.length <= MAX_JUMP_HOPS) {
    const hop = hosts.find((h) => h.id === next);
    if (!hop || chain.includes(hop)) break;
    chain.unshift(hop);
    next = hop.jump_host_id;
  }
  return chain;
}

/** How many hosts deep the hosts going through `id` reach (0 if none). */
export function jumpDepthBelow(hosts: Pick<SshHost, 'id' | 'jump_host_id'>[], id: string): number {
  let level = [id];
  const seen = new Set(level);
  let depth = 0;
  for (;;) {
    const next = hosts
      .filter((h) => h.jump_host_id && level.includes(h.jump_host_id) && !seen.has(h.id))
      .map((h) => h.id);
    if (next.length === 0) return depth;
    next.forEach((child) => seen.add(child));
    depth += 1;
    level = next;
  }
}

export interface SshHostInput {
  name: string;
  host: string;
  port?: number;
  username: string;
  auth_method?: SshAuthMethod;
  key_path?: string | null;
  /** 'content' = the pasted key saved in the vault (SecretsInput.private_key). */
  key_source?: SshKeySource;
  identity_id?: string | null;
  key_id?: string | null;
  jump_host_id?: string | null;
  use_system_ssh?: boolean;
  agent_forwarding?: boolean;
  default_dir?: string | null;
  color?: string | null;
  tags?: string[];
}

export type SecretChange =
  | { action: 'keep' }
  | { action: 'set'; value: string }
  | { action: 'clear' };

export interface SecretsInput {
  store: SshSecretStore;
  password?: SecretChange;
  passphrase?: SecretChange;
  private_key?: SecretChange;
}

export type SshStage = 'network' | 'host_key' | 'auth';

export interface SshTestReport {
  ok: boolean;
  failed_stage?: SshStage | null;
  error?: string | null;
}

export interface ChangedKey {
  known_hosts_path: string;
  line: number;
  old_fingerprint?: string | null;
}

export interface HostKeyPrompt {
  host_id: string;
  label: string;
  algorithm: string;
  fingerprint: string;
  changed?: ChangedKey | null;
}

export interface HostKeyDecision {
  trust: boolean;
  remember: boolean;
}

export type AuthPromptKind = 'password' | 'passphrase' | 'keyboard_interactive';

export interface PromptField {
  text: string;
  echo: boolean;
}

export interface AuthPrompt {
  host_id: string;
  label: string;
  kind: AuthPromptKind;
  name: string;
  instructions: string;
  prompts: PromptField[];
  storable: boolean;
  store: SshSecretStore;
  error?: string | null;
  attempt: number;
}

export interface AuthReply {
  answers: string[];
  store: SshSecretStore;
}

export interface ConfigHost {
  alias: string;
  host_name?: string | null;
  port?: number | null;
  user?: string | null;
  identity_file?: string | null;
  proxy_jump?: string | null;
  forward_agent: boolean;
  unsupported: string[];
}

export interface ConfigScan {
  path?: string | null;
  hosts: ConfigHost[];
}

export type SshConnState =
  | 'connecting'
  | 'verifying'
  | 'authenticating'
  | 'ready'
  | 'lost'
  | 'closed';

export interface SshStatusEvent {
  host_id: string;
  state: SshConnState;
  error?: string | null;
}

// --- Keychain (Termius style) ---

export interface SshKey {
  id: string;
  name: string;
  algorithm: string;
  /** OpenSSH public key line, ready for authorized_keys. */
  public_key: string;
  fingerprint: string;
  /** The stored key still needs its passphrase. */
  encrypted: boolean;
  secret_store: Exclude<SshSecretStore, 'none'>;
  created_at: string;
}

export interface SshKeyView extends SshKey {
  used_by_hosts: number;
  used_by_identities: number;
}

export interface SshIdentity {
  id: string;
  name: string;
  username: string;
  key_id?: string | null;
  /** 'none' = no saved password; the login dialog asks. */
  secret_store: SshSecretStore;
  created_at: string;
}

export interface SshIdentityView extends SshIdentity {
  used_by_hosts: number;
}

export interface SshIdentityInput {
  name: string;
  username: string;
  key_id?: string | null;
}

export interface KeychainList {
  keys: SshKeyView[];
  identities: SshIdentityView[];
}
