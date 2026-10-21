export interface CloudAccountStatus {
  available: boolean;
  problem?: string;
  signedIn: boolean;
  workspaceId?: string;
  phoneConnected?: boolean;
  controlsEnabled?: boolean;
}
export interface CloudAccountActions {
  status(): Promise<CloudAccountStatus>;
  verifySession(): Promise<CloudAccountStatus>;
  subscribe(listener: () => void): () => void;
  onSignOut(listener: () => void): () => void;
  signIn(): Promise<boolean>;
  request(
    path: string,
    method?: "GET" | "POST" | "DELETE" | "PUT",
    body?: unknown,
  ): Promise<any>;
  serviceRequest(
    workspaceId: string,
    path: string,
    method?: "GET" | "PUT" | "POST",
    body?: unknown,
  ): Promise<any>;
  hostingRequest(
    workspaceId: string,
    path: string,
    method?: "GET" | "POST" | "DELETE",
    body?: unknown,
  ): Promise<any>;
  connectPhone(controls: boolean): Promise<void>;
  signOut(): Promise<{ revoked: boolean }>;
}
