// Normalizes a refused policy change into what the Settings sections render.
//
// A policy rejection never names a field, so it cannot go through fieldError:
// the server reports it as a validation report over the whole project, and the
// useful detail is which stored tasks stand in the way. Issues that name no
// task are kept too, because a project that fails to load its policy at all
// reports itself in exactly that shape.

import { ApiError } from "../api/client.js";

export function policyRejection(error) {
  if (!(error instanceof ApiError)) {
    return { message: error.message, blockingTasks: [], allowed: [], issues: [] };
  }
  const blockingTasks = [];
  const issues = [];
  for (const issue of error.issues) {
    const task = issue.context && issue.context.task;
    if (task && !blockingTasks.includes(task)) {
      blockingTasks.push(task);
    }
    issues.push({ code: issue.code, message: issue.message, path: issue.path ?? null });
  }
  return { message: error.message, blockingTasks, allowed: allowedFrom(error), issues };
}

// One rejection is held at a time, tagged with its section, so a refusal can
// only ever appear under the control the user just used.
export function sectionRejection(rejection, section) {
  return rejection && rejection.section === section ? rejection : null;
}

function allowedFrom(error) {
  const topLevel = error.context && error.context.allowed;
  if (Array.isArray(topLevel)) {
    return topLevel;
  }
  for (const issue of error.issues) {
    const allowed = issue.context && issue.context.allowed;
    if (Array.isArray(allowed)) {
      return allowed;
    }
  }
  return [];
}
