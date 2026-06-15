"use strict";

const rows = document.getElementById("task-rows");
const listEmpty = document.getElementById("list-empty");
const detail = document.getElementById("detail");
const detailPlaceholder = document.getElementById("detail-placeholder");
const errorBox = document.getElementById("error");

let selectedId = null;

function showError(message) {
  errorBox.textContent = message;
  errorBox.classList.remove("hidden");
  setTimeout(() => errorBox.classList.add("hidden"), 5000);
}

async function request(method, url, body) {
  const options = { method };
  if (body !== undefined) {
    options.headers = { "content-type": "application/json" };
    options.body = JSON.stringify(body);
  }
  const response = await fetch(url, options);
  const text = await response.text();
  const payload = text ? JSON.parse(text) : null;
  if (!response.ok) {
    const detail = payload && payload.error ? payload.error : response.statusText;
    throw new Error(detail);
  }
  return payload;
}

function queryString() {
  const params = new URLSearchParams();
  for (const id of ["status", "priority", "domain", "milestone", "tag", "sort_by"]) {
    const value = document.getElementById(id).value.trim();
    if (value) {
      params.set(id, value);
    }
  }
  const query = params.toString();
  return query ? `?${query}` : "";
}

async function loadTasks() {
  try {
    const tasks = await request("GET", `/api/tasks${queryString()}`);
    renderRows(tasks);
  } catch (error) {
    showError(`Could not load tasks: ${error.message}`);
  }
}

function renderRows(tasks) {
  rows.replaceChildren();
  listEmpty.classList.toggle("hidden", tasks.length > 0);
  for (const task of tasks) {
    const row = document.createElement("tr");
    if (task.id === selectedId) {
      row.classList.add("selected");
    }
    for (const field of [task.id, task.title, task.status, task.priority, task.domain]) {
      const cell = document.createElement("td");
      cell.textContent = field ?? "";
      row.appendChild(cell);
    }
    row.addEventListener("click", () => loadDetail(task.id));
    rows.appendChild(row);
  }
}

async function loadDetail(id) {
  try {
    const view = await request("GET", `/api/tasks/${encodeURIComponent(id)}`);
    selectedId = id;
    renderDetail(view.detail);
    highlightSelected();
  } catch (error) {
    showError(`Could not load ${id}: ${error.message}`);
  }
}

function highlightSelected() {
  for (const row of rows.children) {
    const id = row.firstChild ? row.firstChild.textContent : "";
    row.classList.toggle("selected", id === selectedId);
  }
}

function renderDetail(detailView) {
  const task = detailView.task;
  detailPlaceholder.classList.add("hidden");
  detail.classList.remove("hidden");
  detail.replaceChildren();

  const heading = document.createElement("h2");
  heading.textContent = `${task.id} · ${task.title}`;
  detail.appendChild(heading);

  const meta = document.createElement("div");
  meta.className = "meta";
  meta.textContent = `${task.status} · ${task.priority} · ${task.type} · ${task.domain}`;
  detail.appendChild(meta);

  detail.appendChild(buildActions(task));

  if (detailView.criteria.length > 0) {
    const list = document.createElement("ul");
    list.className = "criteria";
    for (const item of detailView.criteria) {
      const li = document.createElement("li");
      if (item.checked) {
        li.classList.add("checked");
      }
      li.textContent = item.text;
      list.appendChild(li);
    }
    detail.appendChild(list);
  }

  for (const section of detailView.sections) {
    const wrapper = document.createElement("div");
    wrapper.className = "section";
    const title = document.createElement("h3");
    title.textContent = section.heading;
    const body = document.createElement("pre");
    body.textContent = section.content.trim();
    wrapper.append(title, body);
    detail.appendChild(wrapper);
  }
}

const TRANSITIONS = [
  { action: "start", label: "Start" },
  { action: "done", label: "Done" },
  { action: "block", label: "Block", reason: true },
  { action: "defer", label: "Defer", reason: true },
  { action: "obsolete", label: "Obsolete", reason: true },
];

function buildActions(task) {
  const container = document.createElement("div");
  container.className = "actions";
  for (const transition of TRANSITIONS) {
    const button = document.createElement("button");
    button.textContent = transition.label;
    button.addEventListener("click", () => runTransition(task.id, transition));
    container.appendChild(button);
  }
  return container;
}

async function runTransition(id, transition) {
  let body;
  if (transition.reason) {
    const reason = window.prompt(`Reason to ${transition.label.toLowerCase()} ${id}:`);
    if (!reason) {
      return;
    }
    body = { reason };
  }
  try {
    const view = await request(
      "POST",
      `/api/tasks/${encodeURIComponent(id)}/${transition.action}`,
      body,
    );
    renderDetail(view.detail);
    await loadTasks();
  } catch (error) {
    showError(`Could not ${transition.action} ${id}: ${error.message}`);
  }
}

document.getElementById("refresh").addEventListener("click", loadTasks);
for (const id of ["status", "priority", "sort_by"]) {
  document.getElementById(id).addEventListener("change", loadTasks);
}
loadTasks();
