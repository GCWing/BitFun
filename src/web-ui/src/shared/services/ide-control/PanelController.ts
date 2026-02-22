/**
 * Panel controller.
 *
 * Implements a subset of IDE control operations focused on opening/closing panels.
 */
import { i18nService } from '@/infrastructure/i18n';
import {
  IdeController,
  IdeControlEvent,
  IdeControlOptions,
  PanelType,
  PanelConfig,
  PanelOpenConfig,
} from './types';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('PanelController');

 
export class PanelController implements IdeController {
   
  async execute(
    target: IdeControlEvent['target'],
    options?: IdeControlOptions,
    metadata?: IdeControlEvent['metadata']
  ): Promise<void> {
    const panelType = target.type as PanelType;
    const config = target.config || {};
    const position = target.position || 'right';

    
    const openConfig: PanelOpenConfig = {
      panelType,
      position,
      config,
      options,
    };

    
    await this.openPanel(openConfig);

    
    if (metadata?.request_id) {
      this.sendExecutionResult(metadata.request_id, true, `Panel ${panelType} opened successfully`);
    }
  }

   
  async openPanel(config: PanelOpenConfig): Promise<void> {
    const { panelType, position, config: panelConfig, options } = config;

    
    const mode = options?.mode || 'agent';
    const eventName =
      mode === 'project' ? 'project-create-tab' : mode === 'git' ? 'git-create-tab' : 'agent-create-tab';

    
    const tabDetail = this.buildTabDetail(panelType, panelConfig || {}, options);

    
    if (position === 'right' && options?.expand_panel !== false) {
      window.dispatchEvent(new CustomEvent('expand-right-panel'));
      
      
      await this.waitForPanelExpansion();
    }

    
    window.dispatchEvent(
      new CustomEvent(eventName, {
        detail: tabDetail,
      })
    );

    
    if (options?.auto_focus !== false) {
      
      setTimeout(() => {
        this.focusPanel(panelType);
      }, 100);
    }
  }

   
  async closePanel(panelType: PanelType): Promise<void> {
    
    window.dispatchEvent(
      new CustomEvent('ide-close-panel', {
        detail: { panelType },
      })
    );
  }

   
  async togglePanel(panelType: PanelType): Promise<void> {
    
    window.dispatchEvent(
      new CustomEvent('ide-toggle-panel', {
        detail: { panelType },
      })
    );
  }

   
  focusPanel(panelType: PanelType): void {
    
    window.dispatchEvent(
      new CustomEvent('ide-focus-panel', {
        detail: { panelType },
      })
    );
  }

   
  private buildTabDetail(
    panelType: PanelType,
    config: PanelConfig,
    options?: IdeControlOptions
  ): any {
    const duplicateCheckKey = this.getDuplicateCheckKey(panelType, config);
    const baseDetail = {
      type: panelType,
      title: this.getPanelTitle(panelType, config),
      data: config.data || {},
      metadata: {
        duplicateCheckKey,
      },
      checkDuplicate: options?.check_duplicate ?? true,
      replaceExisting: options?.replace_existing ?? false,
      duplicateCheckKey, 
    };

    
    switch (panelType) {
      case 'git-diff':
        return {
          ...baseDetail,
          data: {
            ...baseDetail.data,
            filePath: config.file_path,
            diffType: config.diff_type || 'staged',
          },
        };

      case 'git-settings':
        return {
          ...baseDetail,
          title: i18nService.getT()('common:tabs.gitSettings'),
        };

      case 'planner':
        return {
          ...baseDetail,
          title: i18nService.getT()('common:tabs.taskPlanner'),
        };

      case 'mermaid-editor':
        return {
          ...baseDetail,
          title: config.title || i18nService.getT()('common:tabs.mermaidChart'),
          data: {
            
            mermaid_code: config.mermaid_code,
            sourceCode: config.mermaid_code || config.sourceCode, 
            mode: config.mode || 'interactive',
            session_id: config.session_id,
            allow_mode_switch: config.allow_mode_switch !== false,
            editor_config: config.editor_config,
            interactive_config: config.interactive_config,
          },
        };

      default:
        return baseDetail;
    }
  }

   
  private getPanelTitle(panelType: PanelType, config: PanelConfig): string {
    const t = i18nService.getT();
    switch (panelType) {
      case 'git-settings':
        return t('common:tabs.gitSettings');
      case 'git-diff':
        return config.file_path ? `${t('common:tabs.gitDiff')}: ${config.file_path}` : t('common:tabs.gitDiff');
      case 'planner':
        return t('common:tabs.taskPlanner');
      case 'file-viewer':
        return t('common:tabs.fileBrowser');
      case 'code-editor':
        return config.file_path || t('common:tabs.editor');
      case 'markdown-editor':
        return config.file_path || t('common:tabs.markdown');
      case 'mermaid-editor':
        return config.title || t('common:tabs.mermaidChart');
      default:
        return panelType;
    }
  }

   
  private getDuplicateCheckKey(panelType: PanelType, config: PanelConfig): string {
    switch (panelType) {
      case 'git-diff':
        return config.file_path ? `git-diff-${config.file_path}` : 'git-diff';
      case 'code-editor':
      case 'markdown-editor':
        return config.file_path ? config.file_path : `${panelType}-${Date.now()}`;
      case 'mermaid-editor':
        return config.session_id ? `mermaid-${config.session_id}` : `mermaid-${Date.now()}`;
      case 'planner':
      case 'git-settings':
        
        return panelType;
      default:
        return `${panelType}-${Date.now()}`;
    }
  }

   
  private async waitForPanelExpansion(): Promise<void> {
    return new Promise((resolve) => {
      
      setTimeout(resolve, 300);
    });
  }

   
  private sendExecutionResult(requestId: string, success: boolean, message: string): void {
    
    import('@tauri-apps/api/core').then(({ invoke }) => {
      invoke('report_ide_control_result', {
        request_id: requestId,
        success,
        message: success ? message : undefined,
        error: success ? undefined : message,
        timestamp: Date.now(),
      }).catch((error) => {
        log.error('Failed to send execution result', error);
      });
    });
  }
}
