 

import React from 'react';
import { FileIcon, Code, Network, Code2 as Code2Icon } from 'lucide-react';
import { contextRegistry } from '../../services/ContextRegistry';
import { 
  FileContextTransformer, 
  FileContextValidator, 
  FileCardRenderer 
} from './types/FileContextImpl';
import { 
  CodeSnippetContextTransformer, 
  CodeSnippetContextValidator, 
  CodeSnippetCardRenderer 
} from './types/CodeSnippetContextImpl';
import { 
  MermaidDiagramContextTransformer, 
  MermaidDiagramContextValidator, 
  MermaidDiagramCardRenderer 
} from './types/MermaidDiagramContextImpl';
import { 
  ImageContextTransformer, 
  ImageContextValidator, 
  ImageCardRenderer 
} from './types/ImageContextImpl';
import {
  WebElementContextTransformer,
  WebElementContextValidator,
  WebElementCardRenderer,
} from './types/WebElementContextImpl';
import { i18nService } from '@/infrastructure/i18n';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('ContextRegistry');

 
export function registerDefaultContextTypes(): void {
  let registeredCount = 0;
  
  try {
    
    contextRegistry.register({
      type: 'file',
      displayName: i18nService.t('components:contextSystem.contextRegistry.file.name'),
      description: i18nService.t('components:contextSystem.contextRegistry.file.description'),
      icon: React.createElement(FileIcon, { size: 16 }),
      color: '#60a5fa', 
      category: 'file',
      transformer: new FileContextTransformer(),
      validator: new FileContextValidator(),
      renderer: new FileCardRenderer(),
      config: {
        maxSize: 50 * 1024 * 1024, // 50MB
        cacheable: true,
        priority: 1
      }
    });
    registeredCount++;
  } catch (error) {
    log.error('Failed to register file type', error as Error);
  }
  
  try {
    
    contextRegistry.register({
      type: 'directory',
      displayName: i18nService.t('components:contextSystem.contextRegistry.directory.name'),
      description: i18nService.t('components:contextSystem.contextRegistry.directory.description'),
      icon: React.createElement(FileIcon, { size: 16 }),
      color: '#8b5cf6', 
      category: 'file',
      transformer: new FileContextTransformer() as any,
      validator: new FileContextValidator() as any,
      renderer: new FileCardRenderer() as any,
      config: {
        cacheable: true,
        priority: 2
      }
    });
    registeredCount++;
  } catch (error) {
    log.error('Failed to register directory type', error as Error);
  }
  
  try {
    
    contextRegistry.register({
      type: 'code-snippet',
      displayName: i18nService.t('components:contextSystem.contextRegistry.codeSnippet.name'),
      description: i18nService.t('components:contextSystem.contextRegistry.codeSnippet.description'),
      icon: React.createElement(Code, { size: 16 }),
      color: '#a78bfa', 
      category: 'code',
      transformer: new CodeSnippetContextTransformer(),
      validator: new CodeSnippetContextValidator(),
      renderer: new CodeSnippetCardRenderer(),
      config: {
        maxSize: 100000, // 100KB
        cacheable: false,
        priority: 5
      }
    });
    registeredCount++;
  } catch (error) {
    log.error('Failed to register code-snippet type', error as Error);
  }
  
  try {
    
    contextRegistry.register({
      type: 'mermaid-diagram',
      displayName: i18nService.t('components:contextSystem.contextRegistry.mermaidDiagram.name'),
      description: i18nService.t('components:contextSystem.contextRegistry.mermaidDiagram.description'),
      icon: React.createElement(Network, { size: 16 }),
      color: '#22c55e', 
      category: 'diagram',
      transformer: new MermaidDiagramContextTransformer(),
      validator: new MermaidDiagramContextValidator(),
      renderer: new MermaidDiagramCardRenderer(),
      config: {
        maxSize: 50000, 
        cacheable: false,
        priority: 4
      }
    });
    registeredCount++;
  } catch (error) {
    log.error('Failed to register mermaid-diagram type', error as Error);
  }
  
  try {
    
    contextRegistry.register({
      type: 'image',
      displayName: i18nService.t('components:contextSystem.contextRegistry.image.name'),
      description: i18nService.t('components:contextSystem.contextRegistry.image.description'),
      icon: React.createElement(FileIcon, { size: 16 }),
      color: '#f59e0b', 
      category: 'media',
      transformer: new ImageContextTransformer(),
      validator: new ImageContextValidator(),
      renderer: new ImageCardRenderer(),
      config: {
        maxSize: 20 * 1024 * 1024, // 20MB
        cacheable: true,
        priority: 3
      }
    });
    registeredCount++;
  } catch (error) {
    log.error('Failed to register image type', error as Error);
  }
  
  try {
    contextRegistry.register({
      type: 'web-element',
      displayName: i18nService.t('components:contextSystem.contextRegistry.webElement.name'),
      description: i18nService.t('components:contextSystem.contextRegistry.webElement.description'),
      icon: React.createElement(Code2Icon, { size: 16 }),
      color: '#38bdf8',
      category: 'reference',
      transformer: new WebElementContextTransformer(),
      validator: new WebElementContextValidator(),
      renderer: new WebElementCardRenderer(),
      config: {
        maxSize: 50000,
        cacheable: false,
        priority: 6,
      },
    });
    registeredCount++;
  } catch (error) {
    log.error('Failed to register web-element type', error as Error);
  }

  const registeredTypes = contextRegistry.getAllTypes();
  log.info('Default context types registered', { count: registeredCount, types: registeredTypes });
}

