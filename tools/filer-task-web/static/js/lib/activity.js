// Readable labels for membership actions in the activity log. Task actions
// keep their machine names, which already read as verbs, such as task.done.
const LABELS = Object.freeze({
  "member.grant": "granted access",
  "member.role": "changed role",
  "member.revoke": "removed member",
  "access.request": "requested access",
  "access.deny": "denied access",
});

export function activityLabel(action) {
  return LABELS[action] ?? action;
}
