export interface Env {
  ACCOUNTS: DurableObjectNamespace;
  PLATFORM: DurableObjectNamespace;
  OBJECTS?: R2Bucket;
  PUBLIC_ORIGIN: string;
  CONTENT_DOMAIN: string;
  MEMBERS_JSON: string;
  COMPUTE_POLICY_JSON?: string;
  COMPUTE_CONTROLLER_TOKEN?: string;
  TOKEN_FACTORY_POLICY_JSON?: string;
  TOKEN_FACTORY_KEY?: string;
  COMPOSIO_PROJECT_KEY?: string;
  HOSTING_WORKER_TOKEN?: string;
  HOSTING_WORKER_ORIGIN?: string;
  HOSTING_SECRETS_KEY?: string;
}
