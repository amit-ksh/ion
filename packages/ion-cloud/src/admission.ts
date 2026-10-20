import { CloudFailure, type Account } from "./auth";

export type MemberRole = "owner" | "judge";
export interface Member {
  subject: string;
  role: MemberRole;
}
/** An empty or absent production allowlist admits nobody. Email and token role hints are ignored. */
export class Admission {
  private readonly members = new Map<string, MemberRole>();
  constructor(input: unknown) {
    if (!Array.isArray(input) || input.length > 64)
      throw new Error("Configure up to 64 approved native user subjects");
    for (const value of input) {
      if (
        !value ||
        typeof value.subject !== "string" ||
        !value.subject.trim() ||
        value.subject.length > 512 ||
        /[\r\n\0]/.test(value.subject) ||
        !["owner", "judge"].includes(value.role) ||
        this.members.has(value.subject)
      )
        throw new Error("Invalid or duplicate approved account");
      this.members.set(value.subject, value.role);
    }
  }
  admit(account: Account): MemberRole {
    const role = this.members.get(account.subject);
    if (!role)
      throw new CloudFailure(
        403,
        "This account is not enabled for the Ion pilot",
      );
    return role;
  }
  owner(account: Account): void {
    if (this.admit(account) !== "owner")
      throw new CloudFailure(403, "Owner permission required");
  }
}
