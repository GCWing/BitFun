import { ConversationUiPlanTool, ConversationUiToolStatus } from '../state/ConversationUiModels';

interface PlanToolInput {
  plan_file_path?: string;
  file_path?: string;
  filePath?: string;
  path?: string;
  name?: string;
  title?: string;
  overview?: string;
}

/** Pure recognition/projection for legacy CreatePlan and modern .plan.md writes. */
export class PlanToolPolicy {
  static descriptor(tool: ConversationUiToolStatus): ConversationUiPlanTool | undefined {
    if (tool.plan) {
      return {
        file_path: tool.plan.file_path || '',
        name: PlanToolPolicy.planName(tool.plan.name, tool.plan.file_path),
        overview: tool.plan.overview
      };
    }
    const input = PlanToolPolicy.input(tool);
    const path = input.plan_file_path || input.file_path || input.filePath || input.path || '';
    const normalizedName = PlanToolPolicy.normalizedName(tool.name || '');
    const isCreatePlan = normalizedName === 'createplan';
    const isPlanWrite = (normalizedName === 'write' || normalizedName === 'writefile' ||
      normalizedName === 'createfile') && path.toLowerCase().endsWith('.plan.md');
    if (!isCreatePlan && !isPlanWrite) {
      return undefined;
    }
    return {
      file_path: path,
      name: PlanToolPolicy.planName(input.name || input.title || '', path),
      overview: input.overview
    };
  }

  static isPlanTool(tool: ConversationUiToolStatus): boolean {
    return PlanToolPolicy.descriptor(tool) !== undefined;
  }

  static canBuild(tool: ConversationUiToolStatus): boolean {
    const plan = PlanToolPolicy.descriptor(tool);
    const status = (tool.status || '').toLowerCase();
    return !!plan && plan.file_path.length > 0 &&
      (status === 'completed' || status === 'done' || status === 'success');
  }

  private static normalizedName(name: string): string {
    return name.replace(/[\s_-]/g, '').toLowerCase();
  }

  private static input(tool: ConversationUiToolStatus): PlanToolInput {
    if (tool.tool_input) {
      try {
        return JSON.parse(JSON.stringify(tool.tool_input)) as PlanToolInput;
      } catch (_err) {
      }
    }
    const preview = tool.input_preview || '';
    if (preview.trim().length > 0) {
      try {
        return JSON.parse(preview) as PlanToolInput;
      } catch (_err) {
      }
    }
    return {};
  }

  private static planName(explicitName: string, path: string): string {
    if (explicitName.trim().length > 0) {
      return explicitName.trim();
    }
    const normalizedPath = path.replace(/\\/g, '/');
    const parts = normalizedPath.split('/');
    const fileName = parts[parts.length - 1] || '';
    if (fileName.toLowerCase().endsWith('.plan.md')) {
      return fileName.slice(0, fileName.length - '.plan.md'.length);
    }
    return fileName || 'Plan';
  }
}
