// Single fetch wrapper: every API call scopes its path to the active project
// and parses the structured error body shape from src/error.rs.
export class ApiError extends Error {
  constructor(status, body) {
    super(body && body.error ? body.error : `request failed with status ${status}`);
    this.status = status;
    this.code = body && body.code ? body.code : null;
    this.field = body && body.field ? body.field : null;
    this.context = body && body.context ? body.context : null;
    this.project = body && body.project ? body.project : null;
    this.issues = body && Array.isArray(body.issues) ? body.issues : [];
  }
}

async function request(method, path, body, headers) {
  const options = { method };
  if (headers) {
    options.headers = { ...headers };
  }
  if (body !== undefined) {
    options.headers = { ...(options.headers || {}), "content-type": "application/json" };
    options.body = JSON.stringify(body);
  }
  const response = await fetch(path, options);
  const text = await response.text();
  const payload = text ? JSON.parse(text) : null;
  if (!response.ok) {
    throw new ApiError(response.status, payload);
  }
  return payload;
}

export function getJson(path) {
  return request("GET", path);
}

export function postJson(path, body) {
  return request("POST", path, body ?? {});
}

export function putJson(path, body, headers) {
  return request("PUT", path, body ?? {}, headers);
}

export function getIdentity() {
  return getJson("/api/identity");
}

export function putIdentity(username) {
  return putJson("/api/identity", { username });
}

// Root-level endpoint, not project-scoped.
export function listProjects() {
  return getJson("/api/projects");
}

export function projectScoped(projectName) {
  const base = `/api/projects/${encodeURIComponent(projectName)}`;
  return {
    getTasks(query) {
      return getJson(`${base}/tasks${toQueryString(query)}`);
    },
    getReady(query) {
      return getJson(`${base}/ready${toQueryString(query)}`);
    },
    getMilestones() {
      return getJson(`${base}/milestones`);
    },
    getPolicy() {
      return getJson(`${base}/policy`);
    },
    getTask(id) {
      return getJson(`${base}/tasks/${encodeURIComponent(id)}`);
    },
    createTask(body) {
      return postJson(`${base}/tasks`, body);
    },
    setCriterion(id, index, checked, contentHash) {
      return putJson(
        `${base}/tasks/${encodeURIComponent(id)}/criteria/${index}`,
        { checked },
        { "If-Match": `"${contentHash}"` },
      );
    },
    transition(id, action, reason) {
      return postJson(
        `${base}/tasks/${encodeURIComponent(id)}/${action}`,
        reason !== undefined ? { reason } : undefined,
      );
    },
  };
}

function toQueryString(query) {
  if (!query) {
    return "";
  }
  const params = new URLSearchParams();
  for (const [key, value] of Object.entries(query)) {
    if (value !== undefined && value !== null && value !== "") {
      params.set(key, value);
    }
  }
  const search = params.toString();
  return search ? `?${search}` : "";
}
