// Column sorting for the tasks table. The list endpoint sorts Id, Status and
// Priority itself, so those columns keep the server order instead of being
// re-sorted here: the server's status order is lifecycle order, and a local
// re-sort would silently override it. Everything else has no query param and
// sorts in the client.

export const SERVER_SORT_KEY = { id: "Id", status: "Status", priority: "Priority" };

export function isServerSorted(column) {
  return Object.hasOwn(SERVER_SORT_KEY, column);
}

export function compareRows(a, b, column) {
  switch (column) {
    case "title":
      return a.title.localeCompare(b.title);
    case "type":
      return a.type.localeCompare(b.type);
    case "milestone":
      return (a.milestone ?? "").localeCompare(b.milestone ?? "");
    case "updated":
      return (a.last_updated ?? "").localeCompare(b.last_updated ?? "");
    default:
      return a.qualified_id.localeCompare(b.qualified_id);
  }
}

export function sortRows(rows, column, direction) {
  const ordered = isServerSorted(column)
    ? [...rows]
    : [...rows].sort((a, b) => compareRows(a, b, column));
  return direction === "desc" ? ordered.reverse() : ordered;
}
