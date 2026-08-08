export function sessionRequestSucceeded(sessions) {
  return { sessions, error: null };
}

export function sessionRequestFailed(sessions, error) {
  return { sessions, error };
}
