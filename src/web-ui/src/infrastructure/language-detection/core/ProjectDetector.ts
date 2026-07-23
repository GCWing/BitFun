 

import type { 
  ProjectInfo, 
  ProjectType, 
  FrameworkInfo, 
  DetectionContext,
  ProjectDetectionPlugin 
} from '../types';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('ProjectDetector');

// ============================================================================

// ============================================================================

 
interface ProjectMarker {
   
  configFile: string;
  
   
  projectType: ProjectType;
  
   
  primaryLanguage: string;
  
   
  packageManager?: ProjectInfo['packageManager'];
  
   
  checkFields?: {
    path: string[];
    detect: (value: unknown) => FrameworkInfo | null;
  }[];
}

 
const PROJECT_MARKERS: ProjectMarker[] = [
  
  {
    configFile: 'package.json',
    projectType: 'nodejs',
    primaryLanguage: 'javascript',
    packageManager: 'npm',
    checkFields: [
      {
        path: ['dependencies', 'react'],
        detect: (v) => v ? { name: 'react', version: String(v), source: 'package.json' } : null,
      },
      {
        path: ['dependencies', 'vue'],
        detect: (v) => v ? { name: 'vue', version: String(v), source: 'package.json' } : null,
      },
      {
        path: ['dependencies', '@angular/core'],
        detect: (v) => v ? { name: 'angular', version: String(v), source: 'package.json' } : null,
      },
      {
        path: ['dependencies', 'svelte'],
        detect: (v) => v ? { name: 'svelte', version: String(v), source: 'package.json' } : null,
      },
      {
        path: ['dependencies', 'next'],
        detect: (v) => v ? { name: 'nextjs', version: String(v), source: 'package.json' } : null,
      },
      {
        path: ['devDependencies', 'vite'],
        detect: (v) => v ? { name: 'vite', version: String(v), source: 'package.json' } : null,
      },
      {
        path: ['devDependencies', '@tauri-apps/cli'],
        detect: (v) => v ? { name: 'tauri', version: String(v), source: 'package.json' } : null,
      },
      {
        path: ['devDependencies', 'electron'],
        detect: (v) => v ? { name: 'electron', version: String(v), source: 'package.json' } : null,
      },
    ],
  },
  
  
  {
    configFile: 'Cargo.toml',
    projectType: 'rust',
    primaryLanguage: 'rust',
    packageManager: 'cargo',
  },
  {
    configFile: 'pyproject.toml',
    projectType: 'python',
    primaryLanguage: 'python',
    packageManager: 'poetry',
  },
  {
    configFile: 'requirements.txt',
    projectType: 'python',
    primaryLanguage: 'python',
    packageManager: 'pip',
  },
  {
    configFile: 'setup.py',
    projectType: 'python',
    primaryLanguage: 'python',
    packageManager: 'pip',
  },
  {
    configFile: 'go.mod',
    projectType: 'go',
    primaryLanguage: 'go',
    packageManager: 'go',
  },
  {
    configFile: 'pom.xml',
    projectType: 'maven',
    primaryLanguage: 'java',
    packageManager: 'maven',
  },
  {
    configFile: 'build.gradle',
    projectType: 'gradle',
    primaryLanguage: 'java',
    packageManager: 'gradle',
  },
  {
    configFile: 'build.gradle.kts',
    projectType: 'gradle',
    primaryLanguage: 'kotlin',
    packageManager: 'gradle',
  },
  {
    configFile: '*.csproj',
    projectType: 'dotnet',
    primaryLanguage: 'csharp',
  },
  {
    configFile: '*.sln',
    projectType: 'dotnet',
    primaryLanguage: 'csharp',
  },
  {
    configFile: 'pubspec.yaml',
    projectType: 'flutter',
    primaryLanguage: 'dart',
  },
];

 
const MONOREPO_MARKERS = [
  'pnpm-workspace.yaml',
  'lerna.json',
  'nx.json',
  'rush.json',
  'turbo.json',
];

// ============================================================================

// ============================================================================

 
class ProjectDetector {
  private static instance: ProjectDetector;
  
   
  private cache = new Map<string, ProjectInfo>();
  
   
  private plugins: ProjectDetectionPlugin[] = [];
  
  private constructor() {}
  
   
  public static getInstance(): ProjectDetector {
    if (!ProjectDetector.instance) {
      ProjectDetector.instance = new ProjectDetector();
    }
    return ProjectDetector.instance;
  }
  
  // ===========================================================================
  
  // ===========================================================================
  
   
  public async detect(
    workspacePath: string,
    options?: {
       
      useBackend?: boolean;
       
      forceRefresh?: boolean;
    }
  ): Promise<ProjectInfo> {
    const normalizedPath = this.normalizePath(workspacePath);
    
    
    if (!options?.forceRefresh) {
      const cached = this.cache.get(normalizedPath);
      if (cached) {
        return cached;
      }
    }
    
    
    const context: DetectionContext = {
      workspacePath: normalizedPath,
    };
    
    let result: ProjectInfo;
    
    
    if (options?.useBackend !== false) {
      try {
        result = await this.detectWithBackend(normalizedPath);
      } catch (error) {
        log.warn('Backend detection failed, using frontend fallback', error);
        result = await this.detectWithFrontend(context);
      }
    } else {
      result = await this.detectWithFrontend(context);
    }
    
    
    this.cache.set(normalizedPath, result);
    
    return result;
  }
  
   
  private async detectWithBackend(workspacePath: string): Promise<ProjectInfo> {
    const { invoke } = await import('@tauri-apps/api/core');
    
    const backendResult = await invoke<{
      languages: string[];
      primaryLanguage?: string;
      fileCount: Record<string, number>;
      projectTypes: string[];
      totalFiles: number;
    }>('lsp_detect_project', {
      request: { workspacePath },
    });
    
    
    return {
      type: this.inferProjectType(backendResult.projectTypes),
      primaryLanguage: backendResult.primaryLanguage || backendResult.languages[0] || 'plaintext',
      languages: backendResult.languages,
      languageStats: backendResult.fileCount,
      frameworks: [],
      rootPath: workspacePath,
      configFiles: [],
      isMonorepo: false,
      confidence: 0.9,
    };
  }
  
   
  private async detectWithFrontend(context: DetectionContext): Promise<ProjectInfo> {
    const workspacePath = context.workspacePath!;
    
    
    for (const plugin of this.plugins.sort((a, b) => b.priority - a.priority)) {
      const result = await plugin.detect(context);
      if (result) {
        return result;
      }
    }
    
    
    const configResult = await this.detectByConfigFiles(workspacePath);
    if (configResult) {
      return configResult;
    }
    
    
    return this.createUnknownProject(workspacePath);
  }
  
   
  private async detectByConfigFiles(workspacePath: string): Promise<ProjectInfo | null> {
    try {
      const { workspaceAPI } = await import('@/infrastructure/api');
      
      
      const files = await workspaceAPI.listFiles(workspacePath);
      const fileNames = new Set(files.map((f: { name: string }) => f.name.toLowerCase()));
      
      
      const isMonorepo = MONOREPO_MARKERS.some(marker => fileNames.has(marker.toLowerCase()));
      
      
      let matchedMarker: ProjectMarker | null = null;
      const detectedFrameworks: FrameworkInfo[] = [];
      const configFiles: string[] = [];
      
      for (const marker of PROJECT_MARKERS) {
        const configName = marker.configFile.toLowerCase();
        
        
        if (configName.startsWith('*')) {
          const ext = configName.substring(1);
          const matching = files.find(f => f.name.toLowerCase().endsWith(ext));
          if (matching) {
            matchedMarker = marker;
            configFiles.push(matching.name);
            break;
          }
        } else if (fileNames.has(configName)) {
          matchedMarker = marker;
          configFiles.push(marker.configFile);
          
          
          if (marker.checkFields) {
            try {
              const configPath = `${workspacePath}/${marker.configFile}`;
              const content = await workspaceAPI.readFileContent(configPath);
              const parsed = JSON.parse(content);
              
              for (const field of marker.checkFields) {
                const value = this.getNestedValue(parsed, field.path);
                const framework = field.detect(value);
                if (framework) {
                  detectedFrameworks.push(framework);
                }
              }
            } catch {
              
            }
          }
          
          break;
        }
      }
      
      if (!matchedMarker) {
        return null;
      }
      
      
      let projectType = matchedMarker.projectType;
      if (detectedFrameworks.length > 0) {
        projectType = this.inferProjectTypeFromFrameworks(detectedFrameworks) || projectType;
      }
      
      
      let primaryLanguage = matchedMarker.primaryLanguage;
      if (fileNames.has('tsconfig.json') && primaryLanguage === 'javascript') {
        primaryLanguage = 'typescript';
      }
      
      
      let packageManager = matchedMarker.packageManager;
      if (matchedMarker.configFile === 'package.json') {
        if (fileNames.has('pnpm-lock.yaml')) {
          packageManager = 'pnpm';
        } else if (fileNames.has('yarn.lock')) {
          packageManager = 'yarn';
        } else if (fileNames.has('bun.lockb')) {
          packageManager = 'bun';
        }
      }
      
      return {
        type: projectType,
        primaryLanguage,
        languages: [primaryLanguage],
        languageStats: {},
        frameworks: detectedFrameworks,
        packageManager,
        rootPath: workspacePath,
        configFiles,
        isMonorepo,
        confidence: 0.85,
      };
    } catch (error) {
      log.error('Config file detection failed', error);
      return null;
    }
  }
  
   
  private createUnknownProject(workspacePath: string): ProjectInfo {
    return {
      type: 'unknown',
      primaryLanguage: 'plaintext',
      languages: [],
      languageStats: {},
      frameworks: [],
      rootPath: workspacePath,
      configFiles: [],
      isMonorepo: false,
      confidence: 0.1,
    };
  }
  
  // ===========================================================================
  
  // ===========================================================================
  
   
  private inferProjectType(backendTypes: string[]): ProjectType {
    const typeMap: Record<string, ProjectType> = {
      'nodejs': 'nodejs',
      'rust': 'rust',
      'python': 'python',
      'go': 'go',
      'java': 'java',
    };
    
    for (const t of backendTypes) {
      const mapped = typeMap[t.toLowerCase()];
      if (mapped) {
        return mapped;
      }
    }
    
    return 'unknown';
  }
  
   
  private inferProjectTypeFromFrameworks(frameworks: FrameworkInfo[]): ProjectType | null {
    const frameworkNames = frameworks.map(f => f.name.toLowerCase());
    
    if (frameworkNames.includes('react')) {
      if (frameworkNames.includes('nextjs')) return 'nextjs';
      return 'react';
    }
    if (frameworkNames.includes('vue')) {
      if (frameworkNames.includes('nuxt')) return 'nuxt';
      return 'vue';
    }
    if (frameworkNames.includes('angular')) return 'angular';
    if (frameworkNames.includes('svelte')) return 'svelte';
    if (frameworkNames.includes('tauri')) return 'tauri';
    if (frameworkNames.includes('electron')) return 'electron';
    if (frameworkNames.includes('vite')) return 'vite';
    
    return null;
  }
  
   
  private getNestedValue(obj: any, path: string[]): unknown {
    let current = obj;
    for (const key of path) {
      if (current == null || typeof current !== 'object') {
        return undefined;
      }
      current = current[key];
    }
    return current;
  }
  
   
  private normalizePath(path: string): string {
    return path.replace(/\\/g, '/');
  }
  
  // ===========================================================================
  
  // ===========================================================================
  
   
  public registerPlugin(plugin: ProjectDetectionPlugin): void {
    this.plugins.push(plugin);
    this.plugins.sort((a, b) => b.priority - a.priority);
    log.debug('Plugin registered', { pluginName: plugin.name });
  }
  
  // ===========================================================================
  
  // ===========================================================================
  
   
  public clearCache(): void {
    this.cache.clear();
  }
  
   
  public invalidateCache(workspacePath: string): void {
    const normalizedPath = this.normalizePath(workspacePath);
    this.cache.delete(normalizedPath);
  }
  
   
  public getStats(): {
    cacheSize: number;
    pluginCount: number;
  } {
    return {
      cacheSize: this.cache.size,
      pluginCount: this.plugins.length,
    };
  }
}


export const projectDetector = ProjectDetector.getInstance();
export default ProjectDetector;
