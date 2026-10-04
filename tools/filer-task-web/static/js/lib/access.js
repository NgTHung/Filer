// Permission rules the browser applies before it offers a control. The server
// enforces the same rules on every request; asking here only hides controls a
// request would refuse. Admin governs registration alone, so project actions
// depend on the role and never on the admin flag.

export const ACTIONS = Object.freeze({
  WRITE_TASKS: "write-tasks",
  MANAGE_MEMBERS: "manage-members",
  EDIT_POLICY: "edit-policy",
  REGISTER_PROJECTS: "register-projects",
  REQUEST_ACCESS: "request-access",
});

export function can(role, isAdmin, action) {
  switch (action) {
    case ACTIONS.WRITE_TASKS:
      return role === "owner" || role === "editor";
    case ACTIONS.MANAGE_MEMBERS:
    case ACTIONS.EDIT_POLICY:
      return role === "owner";
    case ACTIONS.REGISTER_PROJECTS:
      return isAdmin === true;
    case ACTIONS.REQUEST_ACCESS:
      return role === null || role === undefined;
    default:
      return false;
  }
}

// Which banner a project screen shows: "request" for a non-member, "pending"
// while a request waits, and null once the user can write.
export function accessBanner(role) {
  if (role === "owner" || role === "editor") {
    return null;
  }
  return role === "pending" ? "pending" : "request";
}

// A project keeps at least one owner, so the last owner's role and removal
// controls stay disabled.
export function isLastOwner(member, members) {
  return member.role === "owner" && members.filter((other) => other.role === "owner").length === 1;
}
