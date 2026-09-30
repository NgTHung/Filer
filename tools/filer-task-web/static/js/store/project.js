// Module-level active-project store. A tiny pub/sub instead of a dependency:
// Preact components subscribe with useProjectStore() and re-render on change.
import { useEffect, useState } from "../../vendor/preact-htm.js";
import { listProjects } from "../api/client.js";

const state = {
  projects: [],
  activeName: null,
  loading: true,
  error: null,
};
const listeners = new Set();

function notify() {
  for (const listener of listeners) {
    listener();
  }
}

export function activeProject() {
  return state.projects.find((project) => project.name === state.activeName) ?? null;
}

export function setActiveProject(name) {
  state.activeName = name;
  notify();
}

export async function loadProjects() {
  state.loading = true;
  state.error = null;
  notify();
  try {
    const projects = await listProjects();
    state.projects = projects;
    if (!state.activeName || !projects.some((project) => project.name === state.activeName)) {
      const preferred = projects.find((project) => !project.broken) ?? projects[0] ?? null;
      state.activeName = preferred ? preferred.name : null;
    }
  } catch (error) {
    state.error = error;
  } finally {
    state.loading = false;
    notify();
  }
}

export function useProjectStore() {
  const [, setTick] = useState(0);
  useEffect(() => {
    const listener = () => setTick((tick) => tick + 1);
    listeners.add(listener);
    return () => listeners.delete(listener);
  }, []);
  return state;
}
