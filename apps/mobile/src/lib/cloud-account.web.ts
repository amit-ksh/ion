import type { CloudAccountActions } from "./cloud-account.types";
export const cloudAccount: CloudAccountActions = {
  request: async () => {
    throw new Error("Managed services require the native app.");
  },
  serviceRequest: async () => {
    throw new Error("Managed services require the native app.");
  },
  hostingRequest: async () => {
    throw new Error("Managed services require the native app.");
  },
  status: async () => ({ available: false, signedIn: false }),
  verifySession: async () => ({ available: false, signedIn: false }),
  subscribe: () => () => {},
  onSignOut: () => () => {},
  signIn: async () => {
    throw new Error("Cloud sign-in is available in the native app.");
  },
  connectPhone: async () => {
    throw new Error("Use the native app to connect this phone.");
  },
  signOut: async () => ({ revoked: true }),
};
