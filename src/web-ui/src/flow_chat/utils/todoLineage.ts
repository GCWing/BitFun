/**
 * Todo lineage resolution — Kahn topological sort + depth computation + cycle rejection.
 *
 * TS port of the Rust reference (`legion_control_tool.rs` `resolve_legion_topology`):
 * - edges point from dependency (parent) to dependent (child)
 * - depth: root = 0, child = parent depth + 1
 * - cycle: when Kahn cannot visit every node, the result falls back to the
 *   original flat order (depth 0 everywhere) and `hasCycle` is true so callers
 *   can render the safe fallback.
 *
 * Rendering safety (never throws):
 * - self-loops and references to unknown ids are ignored for the display graph
 * - id-less todos and duplicate ids stay flat (depth 0)
 */

export interface TodoLineageLike {
  id?: string | number | null;
  dependencies?: Array<string | number | null> | null;
}

export interface TodoLineageItem<T> {
  todo: T;
  depth: number;
}

export interface TodoLineageResult<T> {
  /** Topological order with computed depth; flat original order when a cycle is detected. */
  items: TodoLineageItem<T>[];
  /** True when the dependency graph contains a cycle (result fell back to flat). */
  hasCycle: boolean;
}

export function resolveTodoLineage<T extends TodoLineageLike>(
  todos: T[],
): TodoLineageResult<T> {
  const items: TodoLineageItem<T>[] = todos.map((todo) => ({ todo, depth: 0 }));
  if (todos.length === 0) return { items, hasCycle: false };

  // 1. Collect ids that participate in the graph (first occurrence wins for
  //    duplicate ids). Id-less todos stay flat.
  const ids: string[] = [];
  const idSet = new Set<string>();
  const idIndexOf = new Map<string, number>();
  for (const todo of todos) {
    const id = todo.id == null ? undefined : String(todo.id);
    if (id === undefined || id === '') continue;
    if (idSet.has(id)) continue;
    idSet.add(id);
    idIndexOf.set(id, ids.length);
    ids.push(id);
  }
  if (ids.length === 0) return { items, hasCycle: false };

  // 2. Build edges: dependency (parent) -> todo (child). Self-loops and
  //    unknown references are ignored so rendering never breaks.
  const adjacency: string[][] = ids.map(() => []);
  const inDegree: number[] = ids.map(() => 0);
  for (const todo of todos) {
    const childId = todo.id == null ? undefined : String(todo.id);
    if (childId === undefined || childId === '') continue;
    const deps = todo.dependencies ?? [];
    for (const dep of deps) {
      if (dep == null) continue;
      const depId = String(dep);
      if (depId === childId) continue;
      const parentIndex = idIndexOf.get(depId);
      if (parentIndex === undefined) continue;
      adjacency[parentIndex].push(childId);
      const childIndex = idIndexOf.get(childId);
      if (childIndex !== undefined) {
        inDegree[childIndex] += 1;
      }
    }
  }

  // 3. Kahn topological sort (input order as the deterministic ready order).
  const ready: number[] = [];
  inDegree.forEach((degree, index) => {
    if (degree === 0) ready.push(index);
  });
  const order: number[] = [];
  let head = 0;
  while (head < ready.length) {
    const index = ready[head++];
    order.push(index);
    for (const childId of adjacency[index]) {
      const childIndex = idIndexOf.get(childId);
      if (childIndex === undefined) continue;
      inDegree[childIndex] -= 1;
      if (inDegree[childIndex] === 0) ready.push(childIndex);
    }
  }

  // 4. Cycle rejection: if Kahn could not visit every node, fall back flat.
  if (order.length !== ids.length) {
    return { items, hasCycle: true };
  }

  // 5. Depth: root = 0, child = parent depth + 1. Parents precede children in
  //    topological order so the parent depth is always known. When a todo has
  //    multiple parents, the first dependency in the list wins.
  const parents = new Map<number, number>();
  for (const todo of todos) {
    const childId = todo.id == null ? undefined : String(todo.id);
    if (childId === undefined || childId === '') continue;
    const childIndex = idIndexOf.get(childId);
    if (childIndex === undefined) continue;
    const deps = todo.dependencies ?? [];
    for (const dep of deps) {
      if (dep == null) continue;
      const depId = String(dep);
      if (depId === childId) continue;
      const parentIndex = idIndexOf.get(depId);
      if (parentIndex === undefined) continue;
      if (!parents.has(childIndex)) parents.set(childIndex, parentIndex);
    }
  }
  const depthByIndex: number[] = ids.map(() => 0);
  for (const index of order) {
    const parentIndex = parents.get(index);
    if (parentIndex !== undefined) {
      depthByIndex[index] = depthByIndex[parentIndex] + 1;
    }
  }

  // 6. Reorder items by topological order; id-less and duplicate-id todos
  //    keep their original relative order (flat, depth 0).
  const firstIndexById = new Map<string, number>();
  for (let i = 0; i < todos.length; i++) {
    const id = todos[i].id == null ? undefined : String(todos[i].id);
    if (id === undefined || id === '') continue;
    if (!firstIndexById.has(id)) firstIndexById.set(id, i);
  }
  const ordered: TodoLineageItem<T>[] = [];
  const emittedIds = new Set<string>();
  for (const index of order) {
    const id = ids[index];
    const firstIndex = firstIndexById.get(id);
    if (firstIndex === undefined) continue;
    ordered.push({ todo: todos[firstIndex], depth: depthByIndex[index] });
    emittedIds.add(id);
  }
  for (let i = 0; i < todos.length; i++) {
    const todo = todos[i];
    const id = todo.id == null ? undefined : String(todo.id);
    const isFirstGraphNode = id !== undefined && id !== '' && emittedIds.has(id) && firstIndexById.get(id) === i;
    if (isFirstGraphNode) continue;
    ordered.push({ todo, depth: 0 });
  }

  return { items: ordered, hasCycle: false };
}
