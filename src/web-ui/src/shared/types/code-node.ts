 
/**
 * Code graph node types.
 *
 * Used to represent structured code elements (files, classes, functions, etc.)
 * in a tree/graph shape.
 */
export interface CodeNode {
  id: string;
  name: string;
  node_type: 'File' | 'Directory' | 'Function' | 'Class' | 'Variable' | 'Interface' | 'Module';
  file_path?: string;
  language?: string;
  start_line?: number;
  end_line?: number;
  signature?: string;
  documentation?: string;
  children?: CodeNode[];
  parent?: string;
}
