// Shared parsing for the comma-separated list inputs the forms accept.

export function splitList(value) {
  return value
    .split(",")
    .map((entry) => entry.trim())
    .filter(Boolean);
}
