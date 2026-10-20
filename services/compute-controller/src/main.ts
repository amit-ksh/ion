// The pinned official SDK uses fs-ext; an operator Linux environment is required.
if (process.platform !== "linux") throw new Error("Run the operator compute controller on Linux with the SDK native dependencies built. This controller is not required in desktop/mobile installations.");
await import("./controller.ts");
export {};
