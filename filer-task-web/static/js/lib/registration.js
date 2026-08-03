// Decides what a refused POST /api/projects means for the caller, kept free of
// the fetch and store calls so the find-or-create rules can be tested directly.
//
// Two refusals are not really failures. The server discovers the project root
// by walking up from the path, so a path inside an already-registered project
// comes back as a duplicate name and the caller should just switch to it. And
// asking to create where a project already exists means the caller wanted that
// project: init only means "create if there is nothing here".

// Shows the root the server will build, so the create dialog's two fields
// cannot be misread as "path to the project" and "label for it".
export function projectRootPreview(location, name) {
  const parent = location.trim().replace(/[\\/]+$/, "");
  return `${parent || "…"}/${name.trim() || "…"}`;
}

export function registrationOutcome(error, init) {
  if (error.code === "duplicate_project_name" && error.project) {
    return { action: "switch", project: error.project };
  }
  if (init && error.code === "project_already_exists") {
    return { action: "open" };
  }
  return { action: "reject" };
}
