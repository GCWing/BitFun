type TodoStatus = 'completed' | 'in_progress' | 'pending' | 'cancelled';

export interface TodoLike {
  id?: string | number;
  content?: string;
  status?: TodoStatus | string;
}

export interface TodoRenderItem {
  key: string;
  todo: TodoLike;
}

export function createTodoRenderItems(todos: TodoLike[]): TodoRenderItem[] {
  const idCounts = new Map<string, number>();

  for (const todo of todos) {
    if (todo.id === undefined || todo.id === null) continue;
    const id = String(todo.id);
    idCounts.set(id, (idCounts.get(id) ?? 0) + 1);
  }

  return todos.map((todo, index) => {
    if (todo.id === undefined || todo.id === null) {
      return { key: `todo-${index}`, todo };
    }

    const id = String(todo.id);
    if ((idCounts.get(id) ?? 0) <= 1) {
      return { key: id, todo };
    }

    return { key: `${id}-${index}`, todo };
  });
}
