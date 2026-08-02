// Normalizes a failed write into the shape a form renders beside one input.

import { ApiError } from "../api/client.js";

// The server already names the offending input on the error body, so a form
// only routes it; anything unattributed belongs above the form, not beside an
// arbitrary field.
export function fieldError(error) {
  if (!(error instanceof ApiError)) {
    return { field: null, message: error.message, allowed: [] };
  }
  const allowed = error.context && error.context.allowed;
  return {
    field: error.field ?? null,
    message: error.message,
    allowed: Array.isArray(allowed) ? allowed : [],
  };
}
