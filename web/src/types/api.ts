export interface APIResponse<T> {
  data: T;
  error: boolean;
  reason: string;
}

export interface Installation {
  avatar: string;
  id: number;
  name: string;
}

export interface User {
  id: number;
  active: boolean;
  admin: boolean;
  avatar_url: string;
  created_at: number;
  email: string;
  login: string;
  updated_at: number;
}

export interface Machine {
  name: string;
  owner: string;
  arch: string;
  cpu: number;
  cpu_limit: number;
  cpu_allocated: number;
  labels: string[];
  ram_available: number;
  ram_limit: number;
  ram_allocated: number;
  ram_total: number;
  status: 'online' | 'offline' | 'paused';
  created_at: number;
  last_seen_at: number;
  updated_at: number;
}

export interface Runner {
  name: string;
  machine: string;
  id: number;
  installation_id: number;
  owner: string;
  status: 'pending' | 'registered' | 'idle' | 'busy' | 'completed';
  arch: string;
  cpu: number;
  ram: number;
  group_id: number;
  labels: string[];
  cancelled: number;
  created: number;
  accepted: number;
  started: number;
  stopped: number;
  updated: number;
}

export interface MachineInput {
  name: string;
  arch: string;
  limit?: {
    cpu: number;
    ram: number;
  };
  labels?: string[];
}

export interface MachineWithToken extends Machine {
  token: string;
}
