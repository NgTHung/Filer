// Find-or-create for a project path, shared by the Settings form and the
// project switcher so both leave the app on the opened project. The refusal
// travels back untouched, because the two callers render it differently: one
// beside a field, the other above a section.

import { ApiError, registerProject } from "../api/client.js";
import { loadProjects, setActiveProject } from "../store/project.js";
import { registrationOutcome } from "./registration.js";

export async function openProject(path, init, name) {
  const attempt = await register(path, init, name);
  return attempt.retry ? await register(path, false) : attempt;
}

async function register(path, init, name) {
  try {
    const summary = await registerProject(path, init, name);
    await activate(summary.name);
    return { ok: true };
  } catch (error) {
    if (!(error instanceof ApiError)) {
      return { ok: false, error };
    }
    const outcome = registrationOutcome(error, init);
    if (outcome.action === "switch") {
      await activate(outcome.project);
      return { ok: true };
    }
    // A second attempt cannot loop: it runs with init false, and the retry is
    // only ever offered for init true. A named creation never lands here,
    // because the server owns find-or-create for the directory it makes.
    if (outcome.action === "open") {
      return { ok: false, retry: true };
    }
    return { ok: false, error };
  }
}

// loadProjects only re-picks a default when the active name has disappeared, so
// the switch to a freshly opened project has to be explicit.
async function activate(name) {
  await loadProjects();
  setActiveProject(name);
}
