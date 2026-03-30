/**
 * Monaco LSP adapter.
 *
 * Registers Monaco language feature providers and routes requests through
 * `WorkspaceLspManager`. Instances are model-scoped and managed externally.
 */

import * as monaco from 'monaco-editor';
import { WorkspaceLspManager } from './WorkspaceLspManager';
import { getMonacoLanguage } from '@/infrastructure/language-detection';
import { createLogger } from '@/shared/utils/logger';
import { i18nService } from '@/infrastructure/i18n';
import type { CompletionItem } from '../types';

const log = createLogger('MonacoLspAdapter');


export class GlobalAdapterRegistry {
  static adapters = new Map<string, MonacoLspAdapter>();
  private static registeredLanguages = new Set<string>();

  private static globalProviderDisposables: monaco.IDisposable[] = [];
  
  static register(modelUri: string, adapter: MonacoLspAdapter): void {
    this.adapters.set(modelUri, adapter);
  }
  
  static unregister(modelUri: string): void {
    this.adapters.delete(modelUri);
  }
  
  static get(modelUri: string): MonacoLspAdapter | undefined {
    return this.adapters.get(modelUri);
  }
  
  static getAllUris(): string[] {
    return Array.from(this.adapters.keys());
  }
  
  static isLanguageRegistered(language: string): boolean {
    return this.registeredLanguages.has(language);
  }
  
  static markLanguageRegistered(language: string): void {
    this.registeredLanguages.add(language);
  }


  static addGlobalProvider(provider: monaco.IDisposable): void {
    this.globalProviderDisposables.push(provider);
  }


  static disposeAllProviders(): void {
    for (const disposable of this.globalProviderDisposables) {
      disposable.dispose();
    }
    this.globalProviderDisposables = [];
    this.registeredLanguages.clear();
  }
}

export class MonacoLspAdapter {

  public static diagnosticMode = false;
  
  private model: monaco.editor.ITextModel;
  private editors = new Set<monaco.editor.IStandaloneCodeEditor>();
  private language: string;
  private serverLanguage: string;
  private uri: string;
  private disposables: monaco.IDisposable[] = [];
  private workspaceManager: WorkspaceLspManager;
  

  private completionCache = new Map<string, { items: CompletionItem[]; timestamp: number }>();
  private readonly CACHE_TTL = 5000;

  /** Whether the LSP server has responded successfully at least once. */
  private _serverReady = false;
  /** Timestamp of last server readiness probe (to avoid spamming). */
  private _lastReadinessProbe = 0;
  /** Minimum interval between server readiness probes (ms). */
  private readonly READINESS_PROBE_INTERVAL = 5000;
  

  private pendingCrossFileJump: {
    targetPath: string;
    targetUri: string;
    fileName: string;
    targetLine: number;
    targetColumn: number;
    position: { lineNumber: number; column: number };
  } | null = null;
  
  
  constructor(
    model: monaco.editor.ITextModel,
    language: string,
    filePath: string,
    workspacePath: string
  ) {
    this.model = model;
    

    const modelLanguage = this.model.getLanguageId();
    this.language = modelLanguage || language;
    

    this.serverLanguage = this.resolveServerLanguage(this.language);
    this.uri = this.filePathToUri(filePath);
    this.workspaceManager = WorkspaceLspManager.getOrCreate(workspacePath);


    const modelUri = this.model.uri.toString();
    GlobalAdapterRegistry.register(modelUri, this);

    this.initialize();
  }
  
  
  public registerEditor(editor: monaco.editor.IStandaloneCodeEditor): void {
    if (this.editors.has(editor)) {
      return;
    }
    
    this.editors.add(editor);
    

    this.setupEditorListeners(editor);
  }
  
  
  public unregisterEditor(editor: monaco.editor.IStandaloneCodeEditor): void {
    if (!this.editors.has(editor)) {
      return;
    }
    
    this.editors.delete(editor);
    

  }
  
  
  public getActiveEditor(): monaco.editor.IStandaloneCodeEditor | null {

    const iterator = this.editors.values();
    const result = iterator.next();
    return result.done ? null : result.value;
  }
  
  
  private resolveServerLanguage(language: string): string {

    if (language === 'c') {
      return 'cpp';
    }

    if (language === 'javascript') {
      return 'typescript';
    }
    return language;
  }

  
  private detectLanguageFromPath(filePath: string): string {
    return getMonacoLanguage(filePath);
  }

  
  private filePathToUri(filePath: string): string {
    if (filePath.startsWith('file://')) {
      return filePath;
    }
    

    try {
      if (filePath.includes('%')) {
        filePath = decodeURIComponent(filePath);
      }
    } catch (err) {
      log.warn('Failed to decode path', { filePath, error: err });
    }
    
    let path = filePath.replace(/\\/g, '/');
    if (!path.startsWith('/')) {
      path = '/' + path;
    }
    
    return `file://${path}`;
  }

  
  private async initialize() {
    try {

      this.registerProviders();
      

      setTimeout(() => {
        this.refreshInlayHints();
        this.refreshSemanticTokens();
      }, 500);
    } catch (error) {
      log.error('Failed to initialize', { error });
    }
  }


  
  private registerProviders() {
    const alreadyRegistered = GlobalAdapterRegistry.isLanguageRegistered(this.language);
    
    if (alreadyRegistered) {

      this.registerDiagnosticsListener();
      return;
    }
    
    this.registerCompletionProvider();
    this.registerHoverProvider();
    this.registerDefinitionProvider();
    this.registerReferencesProvider();
    this.registerFormattingProvider();
    this.registerSignatureHelpProvider();
    this.registerRenameProvider();
    this.registerCodeActionProvider();
    this.registerDocumentSymbolProvider();
    this.registerWorkspaceSymbolProvider();
    this.registerDocumentHighlightProvider();
    this.registerInlayHintsProvider();
    this.registerSemanticTokensProvider();
    this.registerDiagnosticsListener();
    
    GlobalAdapterRegistry.markLanguageRegistered(this.language);
  }

  
  private registerCompletionProvider() {
    const provider = monaco.languages.registerCompletionItemProvider(this.language, {
      triggerCharacters: ['.', ':', '<', '"', "'", '/', '@', '#'],
      
      provideCompletionItems: async (model, position) => {

        const modelUri = model.uri.toString();
        const adapter = GlobalAdapterRegistry.get(modelUri);
        
        if (!adapter) {
          return { suggestions: [] };
        }

        return adapter.provideCompletions(model, position);
      }
    });

    GlobalAdapterRegistry.addGlobalProvider(provider);
  }
  
  
  async provideCompletions(_model: monaco.editor.ITextModel, position: monaco.Position) {
    try {

      const cacheKey = `${position.lineNumber}:${position.column}`;
      const cached = this.completionCache.get(cacheKey);
      if (cached && Date.now() - cached.timestamp < this.CACHE_TTL) {
        return {
          suggestions: this.convertCompletions(cached.items)
        };
      }


      const items = await this.workspaceManager.getCompletions(
        this.serverLanguage,
        this.uri,
        position.lineNumber - 1,
        position.column - 1
      );

      // Server responded - mark as ready so hover can work immediately.
      if (items && items.length > 0) {
        this._serverReady = true;
      }

      this.completionCache.set(cacheKey, {
        items,
        timestamp: Date.now()
      });


      this.cleanupCache();

      return {
        suggestions: this.convertCompletions(items)
      };
    } catch (error) {
      log.error('Failed to get completions', { uri: this.uri, position, error });
      return { suggestions: [] };
    }
  }

  
  private registerHoverProvider() {
    const provider = monaco.languages.registerHoverProvider(this.language, {
      provideHover: async (model, position) => {
        const modelUri = model.uri.toString();
        const adapter = GlobalAdapterRegistry.get(modelUri);
        
        if (!adapter) {
          return null;
        }

        return adapter.provideHover(model, position);
      }
    });
    GlobalAdapterRegistry.addGlobalProvider(provider);
  }
  
  
  async provideHover(_model: monaco.editor.ITextModel, position: monaco.Position) {
    // If server has never responded successfully, skip hover to avoid "Loading..." flicker.
    // Periodically re-probe to detect when the server becomes ready.
    if (!this._serverReady) {
      const now = Date.now();
      if (now - this._lastReadinessProbe < this.READINESS_PROBE_INTERVAL) {
        return null;
      }
      this._lastReadinessProbe = now;
    }

    try {
      const hover = await this.workspaceManager.getHover(
        this.serverLanguage,
        this.uri,
        position.lineNumber - 1,
        position.column - 1
      );

      if (!hover) {
        return null;
      }

      // Server responded successfully - mark as ready for future hover requests.
      this._serverReady = true;

      const contents: monaco.IMarkdownString[] = [];
      
      if (hover.contents) {
        if (typeof hover.contents === 'string') {
          contents.push({ value: hover.contents });
        } else if (Array.isArray(hover.contents)) {
          hover.contents.forEach((content: any) => {
            if (typeof content === 'string') {
              contents.push({ value: content });
            } else if (content.value) {
              contents.push({ value: content.value, isTrusted: true });
            }
          });
        } else if (hover.contents.value) {
          contents.push({ value: hover.contents.value, isTrusted: true });
        }
      }

      if (contents.length === 0) {
        return null;
      }

      const result: monaco.languages.Hover = {
        contents,
        range: hover.range ? new monaco.Range(
          hover.range.start.line + 1,
          hover.range.start.character + 1,
          hover.range.end.line + 1,
          hover.range.end.character + 1
        ) : undefined
      };

      return result;
    } catch (error) {
      log.error('Failed to get hover', { uri: this.uri, position, error });
      return null;
    }
  }

  
  private registerDefinitionProvider() {
    const provider = monaco.languages.registerDefinitionProvider(this.language, {
      provideDefinition: async (model, position) => {
        const modelUri = model.uri.toString();
        const adapter = GlobalAdapterRegistry.get(modelUri);
        
        if (!adapter) {
          return null;
        }
        
        return adapter.provideDefinition(model, position);
      }
    });
    
    GlobalAdapterRegistry.addGlobalProvider(provider);
  }
  
  async provideDefinition(_model: monaco.editor.ITextModel, position: monaco.Position) {
    try {
      const definition = await this.workspaceManager.gotoDefinition(
        this.serverLanguage,
        this.uri,
        position.lineNumber - 1,
        position.column - 1
      );


      let definitionItem = definition;
      if (Array.isArray(definition)) {
        if (definition.length === 0) {
          return null;
        }

        definitionItem = definition[0];
      }


      const definitionUri = definitionItem.targetUri || definitionItem.uri;
      if (!definitionUri) {
        return null;
      }

      const currentUri = this.uri;
      

      const normalizeUriToPath = (uri: string): string => {

        let path = uri.replace(/^file:\/+/, '');
        

        path = path.replace(/^\/+([a-zA-Z]:)/, '$1');
        

        path = path.replace(/\\/g, '/');
        

        path = path.replace(/^([a-z]):/, (_match, letter) => letter.toUpperCase() + ':');
        
        return path;
      };
      

      const normalizeForComparison = (path: string): string => {
        return path.toLowerCase();
      };
      
      const definitionPath = normalizeUriToPath(definitionUri);
      const currentPath = normalizeUriToPath(currentUri);
      

      const isSameFile = normalizeForComparison(definitionPath) === normalizeForComparison(currentPath);
      

      const range = definitionItem.targetSelectionRange || definitionItem.targetRange || definitionItem.range;
      if (!range) {
        return null;
      }
      

      const targetMonacoUri = monaco.Uri.parse(definitionUri);
      const location: monaco.languages.Location = {
        uri: targetMonacoUri,
        range: new monaco.Range(
          range.start.line + 1,
          range.start.character + 1,
          range.end.line + 1,
          range.end.character + 1
        )
      };
      
      if (!isSameFile) {

        const targetLine = range.start.line + 1;
        const targetColumn = range.start.character + 1;
        const fileName = definitionPath.split(/[/\\]/).pop() || '';
        

        

        this.pendingCrossFileJump = {
          targetPath: definitionPath,
          targetUri: definitionUri,
          fileName,
          targetLine,
          targetColumn,
          position: {
            lineNumber: position.lineNumber,
            column: position.column
          }
        };
      } else {

        this.pendingCrossFileJump = null;
      }
      

      return location;
    } catch (error) {
      log.error('Failed to goto definition', { uri: this.uri, position, error });
      return null;
    }
  }

  
  private registerReferencesProvider() {
    const provider = monaco.languages.registerReferenceProvider(this.language, {
      provideReferences: async (model, position) => {
        const modelUri = model.uri.toString();
        const adapter = GlobalAdapterRegistry.get(modelUri);
        if (!adapter) return [];
        return adapter.provideReferences(model, position);
      }
    });
    GlobalAdapterRegistry.addGlobalProvider(provider);
  }
  
  async provideReferences(_model: monaco.editor.ITextModel, position: monaco.Position) {
    try {
      const references = await this.workspaceManager.findReferences(
        this.serverLanguage,
        this.uri,
        position.lineNumber - 1,
        position.column - 1
      );

      if (!references || !Array.isArray(references)) return [];


      await this.ensureModelsForReferences(references);

      return references.map((ref: any) => ({
        uri: monaco.Uri.parse(ref.uri),
        range: new monaco.Range(
          ref.range.start.line + 1,
          ref.range.start.character + 1,
          ref.range.end.line + 1,
          ref.range.end.character + 1
        )
      }));
    } catch (error) {
      log.error('Failed to find references', { uri: this.uri, position, error });
      return [];
    }
  }

  
  private async ensureModelsForReferences(references: any[]): Promise<void> {

    const uniqueUris = new Set<string>();
    for (const ref of references) {
      if (ref.uri && ref.uri !== this.uri) {
        uniqueUris.add(ref.uri);
      }
    }

    if (uniqueUris.size === 0) {
      return;
    }


    const loadPromises = Array.from(uniqueUris).map(async (uriString) => {
      try {

        const uri = monaco.Uri.parse(uriString);
        const existingModel = monaco.editor.getModel(uri);
        if (existingModel) {
          return;
        }


        const { normalizePath } = await import('@/shared/utils/pathUtils');
        const filePath = normalizePath(uriString);


        const { workspaceAPI } = await import('@/infrastructure/api');
        const content = await workspaceAPI.readFileContent(filePath);


        const language = this.detectLanguageFromPath(filePath);


        const { monacoModelManager } = await import('@/tools/editor/services/MonacoModelManager');
        monacoModelManager.getOrCreateModel(
          filePath,
          language,
          content,
          this.workspaceManager.getWorkspacePath()
        );
      } catch (error) {
        log.warn('Failed to create model for reference', { uri: uriString, error });

      }
    });


    await Promise.allSettled(loadPromises);
  }

  
  private registerFormattingProvider() {
    const provider = monaco.languages.registerDocumentFormattingEditProvider(this.language, {
      provideDocumentFormattingEdits: async (model) => {
        const modelUri = model.uri.toString();
        const adapter = GlobalAdapterRegistry.get(modelUri);
        if (!adapter) return [];
        return adapter.provideFormatting(model);
      }
    });
    GlobalAdapterRegistry.addGlobalProvider(provider);
  }
  
  async provideFormatting(_model: monaco.editor.ITextModel) {
    try {
      const edits = await this.workspaceManager.formatDocument(
        this.serverLanguage,
        this.uri,
        2,
        true
      );

      if (!edits || !Array.isArray(edits)) return [];

      return edits.map((edit: any) => ({
        range: new monaco.Range(
          edit.range.start.line + 1,
          edit.range.start.character + 1,
          edit.range.end.line + 1,
          edit.range.end.character + 1
        ),
        text: edit.newText
      }));
    } catch (error) {
      log.error('Failed to format document', { uri: this.uri, error });
      return [];
    }
  }

  
  private registerSignatureHelpProvider() {
    const provider = monaco.languages.registerSignatureHelpProvider(this.language, {
      signatureHelpTriggerCharacters: ['(', ','],
      signatureHelpRetriggerCharacters: [')'],
      
      provideSignatureHelp: async (model, position) => {
        const modelUri = model.uri.toString();
        const adapter = GlobalAdapterRegistry.get(modelUri);
        if (!adapter) return null;
        return adapter.provideSignatureHelp(model, position);
      }
    });
    GlobalAdapterRegistry.addGlobalProvider(provider);
  }

  
  private registerRenameProvider() {
    const provider = monaco.languages.registerRenameProvider(this.language, {
      provideRenameEdits: async (model, position, newName) => {
        const modelUri = model.uri.toString();
        const adapter = GlobalAdapterRegistry.get(modelUri);
        if (!adapter) return null;
        return adapter.provideRenameEdits(model, position, newName);
      },
      resolveRenameLocation: async (model, position) => {
        const modelUri = model.uri.toString();
        const adapter = GlobalAdapterRegistry.get(modelUri);
        if (!adapter) return null;
        return adapter.resolveRenameLocation(model, position);
      }
    });
    GlobalAdapterRegistry.addGlobalProvider(provider);
  }

  
  private registerCodeActionProvider() {
    const provider = monaco.languages.registerCodeActionProvider(this.language, {
      provideCodeActions: async (model, range, context) => {
        const modelUri = model.uri.toString();
        const adapter = GlobalAdapterRegistry.get(modelUri);
        if (!adapter) return { actions: [], dispose: () => {} };
        return adapter.provideCodeActions(model, range, context);
      }
    });
    GlobalAdapterRegistry.addGlobalProvider(provider);
  }

  
  private registerDocumentSymbolProvider() {
    const provider = monaco.languages.registerDocumentSymbolProvider(this.language, {
      provideDocumentSymbols: async (model) => {
        const modelUri = model.uri.toString();
        const adapter = GlobalAdapterRegistry.get(modelUri);
        if (!adapter) return [];
        return adapter.provideDocumentSymbols(model);
      }
    });
    GlobalAdapterRegistry.addGlobalProvider(provider);
  }

  
  private registerWorkspaceSymbolProvider() {



  }

  
  private registerDocumentHighlightProvider() {
    const provider = monaco.languages.registerDocumentHighlightProvider(this.language, {
      provideDocumentHighlights: async (model, position) => {
        const modelUri = model.uri.toString();
        const adapter = GlobalAdapterRegistry.get(modelUri);
        if (!adapter) return [];
        return adapter.provideDocumentHighlights(model, position);
      }
    });
    GlobalAdapterRegistry.addGlobalProvider(provider);
  }

  
  private registerInlayHintsProvider() {
    const provider = monaco.languages.registerInlayHintsProvider(this.language, {
      provideInlayHints: async (model, range) => {
        const modelUri = model.uri.toString();
        const adapter = GlobalAdapterRegistry.get(modelUri);
        if (!adapter) {
          return { hints: [], dispose: () => {} };
        }
        return adapter.provideInlayHints(model, range);
      }
    });
    GlobalAdapterRegistry.addGlobalProvider(provider);
  }
  
  
  async provideInlayHints(_model: monaco.editor.ITextModel, range: monaco.Range) {
    try {
      const hints = await this.workspaceManager.getInlayHints(
        this.serverLanguage,
        this.uri,
        range.startLineNumber - 1,
        range.startColumn - 1,
        range.endLineNumber - 1,
        range.endColumn - 1
      );

      if (!hints || !Array.isArray(hints)) {
        return { hints: [], dispose: () => {} };
      }


      const monacoHints: monaco.languages.InlayHint[] = hints.map((hint: any) => {

        let label: string | monaco.languages.InlayHintLabelPart[];
        if (typeof hint.label === 'string') {
          label = hint.label;
        } else if (Array.isArray(hint.label)) {
          label = hint.label.map((part: any) => ({
            label: part.value,
            tooltip: part.tooltip,
            location: part.location,
            command: part.command
          }));
        } else {
          label = String(hint.label);
        }

        return {
          label,
          position: new monaco.Position(
            hint.position.line + 1,
            hint.position.character + 1
          ),
          kind: hint.kind || monaco.languages.InlayHintKind.Type,
          tooltip: hint.tooltip,
          paddingLeft: hint.padding_left || false,
          paddingRight: hint.padding_right || false,
        };
      });

      return {
        hints: monacoHints,
        dispose: () => {}
      };
    } catch (error) {
      log.error('Failed to get inlay hints', { uri: this.uri, range, error });
      return { hints: [], dispose: () => {} };
    }
  }
  
  async provideSignatureHelp(_model: monaco.editor.ITextModel, position: monaco.Position) {
    try {
      const signatureHelp = await this.workspaceManager.getSignatureHelp(
        this.serverLanguage,
        this.uri,
        position.lineNumber - 1,
        position.column - 1
      );

      if (!signatureHelp || !signatureHelp.signatures || signatureHelp.signatures.length === 0) {
        return null;
      }
      

      return {
        value: {
          activeSignature: signatureHelp.activeSignature || 0,
          activeParameter: signatureHelp.activeParameter || 0,
          signatures: signatureHelp.signatures.map((sig: any) => ({
            label: sig.label,
            documentation: sig.documentation,
            parameters: sig.parameters?.map((param: any) => ({
              label: param.label,
              documentation: param.documentation
            })) || []
          }))
        },
        dispose: () => {} // Noop dispose function
      };
    } catch (error) {
      log.error('Failed to get signature help', { uri: this.uri, position, error });
      return null;
    }
  }

  
  async provideRenameEdits(_model: monaco.editor.ITextModel, position: monaco.Position, newName: string) {
    try {
      const edits = await this.workspaceManager.rename(
        this.serverLanguage,
        this.uri,
        position.lineNumber - 1,
        position.column - 1,
        newName
      );

      if (!edits) {
        return null;
      }

      if (!edits.changes && !edits.documentChanges) {
        return null;
      }


      const monacoEdits: any = { edits: [] };
      

      if (edits.changes) {
        for (const [uri, textEdits] of Object.entries(edits.changes)) {
          for (const edit of textEdits as any[]) {
            monacoEdits.edits.push({
              resource: monaco.Uri.parse(uri),
              textEdit: {
                range: new monaco.Range(
                  edit.range.start.line + 1,
                  edit.range.start.character + 1,
                  edit.range.end.line + 1,
                  edit.range.end.character + 1
                ),
                text: edit.newText
              },
              versionId: undefined
            });
          }
        }
      }
      

      if (edits.documentChanges) {
        for (const docChange of edits.documentChanges as any[]) {
          if (docChange.textDocument && docChange.edits) {
            // TextDocumentEdit
            for (const edit of docChange.edits) {
              monacoEdits.edits.push({
                resource: monaco.Uri.parse(docChange.textDocument.uri),
                textEdit: {
                  range: new monaco.Range(
                    edit.range.start.line + 1,
                    edit.range.start.character + 1,
                    edit.range.end.line + 1,
                    edit.range.end.character + 1
                  ),
                  text: edit.newText
                },
                versionId: undefined
              });
            }
          }
        }
      }

      return monacoEdits;
    } catch (error) {
      log.error('Failed to get rename edits', { uri: this.uri, position, newName, error });
      return null;
    }
  }

  
  async resolveRenameLocation(model: monaco.editor.ITextModel, position: monaco.Position) {
    try {

      const word = model.getWordAtPosition(position);
      if (!word) {
        return null;
      }

      return {
        range: new monaco.Range(
          position.lineNumber,
          word.startColumn,
          position.lineNumber,
          word.endColumn
        ),
        text: word.word
      };
    } catch (error) {
      log.error('Failed to resolve rename location', { uri: this.uri, position, error });
      return null;
    }
  }

  
  async provideCodeActions(_model: monaco.editor.ITextModel, range: monaco.Range, context: monaco.languages.CodeActionContext) {
    try {
      
      const actions = await this.workspaceManager.getCodeActions(
        this.serverLanguage,
        this.uri,
        {
          start: { line: range.startLineNumber - 1, character: range.startColumn - 1 },
          end: { line: range.endLineNumber - 1, character: range.endColumn - 1 }
        },
        {
          diagnostics: context.markers.map(m => ({
            range: {
              start: { line: m.startLineNumber - 1, character: m.startColumn - 1 },
              end: { line: m.endLineNumber - 1, character: m.endColumn - 1 }
            },
            severity: m.severity,
            message: m.message,
            code: m.code,
            source: m.source
          })),
          only: context.only ? [context.only] : undefined
        }
      );

      if (!actions || !Array.isArray(actions)) {
        return { actions: [], dispose: () => {} };
      }


      const monacoActions = actions.map((action: any) => ({
        title: action.title,
        kind: action.kind,
        diagnostics: action.diagnostics,
        edit: action.edit ? {
          edits: Object.entries(action.edit.changes || {}).flatMap(([uri, textEdits]: [string, any]) =>
            textEdits.map((edit: any) => ({
              resource: monaco.Uri.parse(uri),
              textEdit: {
                range: new monaco.Range(
                  edit.range.start.line + 1,
                  edit.range.start.character + 1,
                  edit.range.end.line + 1,
                  edit.range.end.character + 1
                ),
                text: edit.newText
              },
              versionId: undefined
            }))
          )
        } : undefined,
        command: action.command
      }));

      return {
        actions: monacoActions,
        dispose: () => {}
      };
    } catch (error) {
      log.error('Failed to get code actions', { uri: this.uri, range, error });
      return { actions: [], dispose: () => {} };
    }
  }

  
  async provideDocumentSymbols(_model: monaco.editor.ITextModel) {
    try {
      
      const symbols = await this.workspaceManager.getDocumentSymbols(
        this.serverLanguage,
        this.uri
      );

      if (!symbols || !Array.isArray(symbols)) {
        return [];
      }


      const convertSymbol = (symbol: any): any => {
        const result: any = {
          name: symbol.name,
          detail: symbol.detail || '',
          kind: symbol.kind,
          range: new monaco.Range(
            symbol.range.start.line + 1,
            symbol.range.start.character + 1,
            symbol.range.end.line + 1,
            symbol.range.end.character + 1
          ),
          selectionRange: new monaco.Range(
            symbol.selectionRange.start.line + 1,
            symbol.selectionRange.start.character + 1,
            symbol.selectionRange.end.line + 1,
            symbol.selectionRange.end.character + 1
          ),
          tags: symbol.tags || []
        };

        if (symbol.children && Array.isArray(symbol.children)) {
          result.children = symbol.children.map(convertSymbol);
        }

        return result;
      };

      const monacoSymbols = symbols.map(convertSymbol);
      return monacoSymbols;
    } catch (error) {
      log.error('Failed to get document symbols', { uri: this.uri, error });
      return [];
    }
  }

  
  async provideWorkspaceSymbols(query: string) {
    try {
      const symbols = await this.workspaceManager.getWorkspaceSymbols(query);

      if (!symbols || !Array.isArray(symbols)) {
        return [];
      }


      const monacoSymbols = symbols.map((symbol: any) => ({
        name: symbol.name,
        kind: symbol.kind,
        containerName: symbol.containerName,
        location: {
          uri: monaco.Uri.parse(symbol.location.uri),
          range: new monaco.Range(
            symbol.location.range.start.line + 1,
            symbol.location.range.start.character + 1,
            symbol.location.range.end.line + 1,
            symbol.location.range.end.character + 1
          )
        }
      }));

      return monacoSymbols;
    } catch (error) {
      log.error('Failed to search workspace symbols', { query, error });
      return [];
    }
  }

  
  async provideDocumentHighlights(_model: monaco.editor.ITextModel, position: monaco.Position) {
    try {
      const highlights = await this.workspaceManager.getDocumentHighlight(
        this.serverLanguage,
        this.uri,
        position.lineNumber - 1,
        position.column - 1
      );

      if (!highlights || !Array.isArray(highlights)) {
        return [];
      }


      const monacoHighlights = highlights.map((highlight: any) => ({
        range: new monaco.Range(
          highlight.range.start.line + 1,
          highlight.range.start.character + 1,
          highlight.range.end.line + 1,
          highlight.range.end.character + 1
        ),
        kind: highlight.kind || monaco.languages.DocumentHighlightKind.Text
      }));

      return monacoHighlights;
    } catch (error) {
      log.error('Failed to get document highlights', { uri: this.uri, position, error });
      return [];
    }
  }

  
  private registerSemanticTokensProvider() {

    const tokenTypes = [
      'comment',        // 0
      'decorator',      // 1
      'enumMember',     // 2
      'enum',           // 3
      'function',       // 4
      'struct',         // 5
      'keyword',        // 6
      'macro',          // 7
      'method',         // 8
      'namespace',      // 9
      'parameter',      // 10
      'operator',       // 11
      'property',       // 12
      'property',       // 13
      'typeParameter',  // 14
      'type',           // 15
      'variable',       // 16
      'class',          // 17
      'interface',      // 18
      'number',         // 19
      'regexp',         // 20
      'modifier',       // 21
      'event',          // 22
      'label',          // 23
      'variable',       // 24
      'property',       // 25
      'function',       // 26
      'method',         // 27
      'macro',          // 28
      'type',           // 29
      'parameter',      // 30
      'property',       // 31
      'variable',       // 32
      'function',       // 33
      'method',         // 34
      'type',           // 35
      'interface',      // 36
      'property',       // 37
      'variable',       // 38
      'parameter',      // 39
      'function',       // 40
      'method',         // 41
      'type',           // 42
      'variable',       // 43
      'property',       // 44
      'enum',           // 45
      'class',          // 46
      'struct',         // 47
      'interface',      // 48
      'typeParameter',  // 49
      'parameter',      // 50
      'variable',       // 51
      'property',       // 52
      'function',       // 53
      'type',           // 54
      'method',         // 55
      'macro',          // 56
      'decorator',      // 57
      'enumMember',     // 58
      'comment',        // 59
    ];
    
    const tokenModifiers = [
      'declaration',    // 0
      'definition',     // 1
      'readonly',       // 2
      'static',         // 3
      'deprecated',     // 4
      'abstract',       // 5
      'async',          // 6
      'modification',   // 7
      'documentation',  // 8
      'defaultLibrary', // 9
    ];
    
    const disposable = monaco.languages.registerDocumentSemanticTokensProvider(
      this.language,
      {
        getLegend: () => {
          return {
            tokenTypes,
            tokenModifiers
          };
        },
        
        provideDocumentSemanticTokens: async (model) => {
          try {
            const modelUri = model.uri.toString();
            const adapter = GlobalAdapterRegistry.get(modelUri);
            
            if (!adapter) {
              return null;
            }
            
            const result = await adapter.provideSemanticTokens(model);
            return result;
          } catch (error) {
            log.error('Failed to provide semantic tokens', { uri: model.uri.toString(), error });
            return null;
          }
        },
        
        releaseDocumentSemanticTokens: (_resultId) => {
          // cleanup
        }
      }
    );
    

    GlobalAdapterRegistry.addGlobalProvider(disposable);
  }

  
  async provideSemanticTokens(_model: monaco.editor.ITextModel) {
    try {
      const semanticTokens = await this.workspaceManager.getSemanticTokens(
        this.serverLanguage,
        this.uri
      );
      
      if (!semanticTokens || !semanticTokens.data) {
        return null;
      }
      

      const resultId = `${this.uri}-${Date.now()}`;
      
      return {
        data: new Uint32Array(semanticTokens.data),
        resultId: semanticTokens.resultId || resultId
      };
    } catch (error) {
      log.error('Failed to get semantic tokens', { uri: this.uri, error });
      return null;
    }
  }


  
  private registerDiagnosticsListener() {

    const listener = (diagnostics: any[]) => {
      // Receiving diagnostics means the server is alive.
      this._serverReady = true;
      this.updateDiagnostics(diagnostics);
    };
    

    this.workspaceManager.onDiagnostics(this.uri, listener);
  }

  
  private updateDiagnostics(diagnostics: any[]) {
    try {
      const markers = diagnostics.map(diag => ({
        severity: this.convertDiagnosticSeverity(diag.severity),
        startLineNumber: diag.range.start.line + 1,
        startColumn: diag.range.start.character + 1,
        endLineNumber: diag.range.end.line + 1,
        endColumn: diag.range.end.character + 1,
        message: diag.message,
        source: diag.source || 'LSP',
        code: diag.code
      }));

      monaco.editor.setModelMarkers(this.model, 'lsp', markers);
    } catch (error) {
      log.error('Failed to update diagnostics', { uri: this.uri, count: diagnostics.length, error });
    }
  }
  
  private convertDiagnosticSeverity(severity?: number): monaco.MarkerSeverity {

    switch (severity) {
      case 1:
        return monaco.MarkerSeverity.Error;
      case 2:
        return monaco.MarkerSeverity.Warning;
      case 3:
        return monaco.MarkerSeverity.Info;
      case 4:
        return monaco.MarkerSeverity.Hint;
      default:
        return monaco.MarkerSeverity.Info;
    }
  }

  
  private convertCompletions(items: CompletionItem[]): monaco.languages.CompletionItem[] {
    return items.map(item => ({
      label: item.label,
      kind: this.convertCompletionKind(item.kind),
      detail: item.detail,
      documentation: item.documentation,
      insertText: item.insertText || item.label,
      range: undefined as any
    }));
  }

  
  private convertCompletionKind(kind?: number): monaco.languages.CompletionItemKind {
    const map: Record<number, monaco.languages.CompletionItemKind> = {
      1: monaco.languages.CompletionItemKind.Text,
      2: monaco.languages.CompletionItemKind.Method,
      3: monaco.languages.CompletionItemKind.Function,
      4: monaco.languages.CompletionItemKind.Constructor,
      5: monaco.languages.CompletionItemKind.Field,
      6: monaco.languages.CompletionItemKind.Variable,
      7: monaco.languages.CompletionItemKind.Class,
      8: monaco.languages.CompletionItemKind.Interface,
      9: monaco.languages.CompletionItemKind.Module,
      10: monaco.languages.CompletionItemKind.Property,
      11: monaco.languages.CompletionItemKind.Unit,
      12: monaco.languages.CompletionItemKind.Value,
      13: monaco.languages.CompletionItemKind.Enum,
      14: monaco.languages.CompletionItemKind.Keyword,
      15: monaco.languages.CompletionItemKind.Snippet,
      16: monaco.languages.CompletionItemKind.Color,
      17: monaco.languages.CompletionItemKind.File,
      18: monaco.languages.CompletionItemKind.Reference,
      19: monaco.languages.CompletionItemKind.Folder,
      20: monaco.languages.CompletionItemKind.EnumMember,
      21: monaco.languages.CompletionItemKind.Constant,
      22: monaco.languages.CompletionItemKind.Struct,
      23: monaco.languages.CompletionItemKind.Event,
      24: monaco.languages.CompletionItemKind.Operator,
      25: monaco.languages.CompletionItemKind.TypeParameter,
    };

    return kind ? map[kind] || monaco.languages.CompletionItemKind.Text : monaco.languages.CompletionItemKind.Text;
  }

  
  private cleanupCache() {
    const now = Date.now();
    for (const [key, value] of this.completionCache.entries()) {
      if (now - value.timestamp > this.CACHE_TTL) {
        this.completionCache.delete(key);
      }
    }
  }

  
  async save() {
    try {
      await this.workspaceManager.saveDocument(this.uri);
    } catch (error) {
      log.error('Failed to save document', { uri: this.uri, error });
    }
  }

  
  private setupEditorListeners(editor: monaco.editor.IStandaloneCodeEditor): void {

    let crossFileDecorations: string[] = [];
    let isCtrlPressed = false;
    let currentHoverPosition: monaco.Position | null = null;
    

    const globalKeyDownHandler = (e: KeyboardEvent) => {
      if (e.ctrlKey || e.metaKey) {
        if (!isCtrlPressed) {
          isCtrlPressed = true;
          

          if (currentHoverPosition) {
            this.checkAndShowCrossFileDecoration(editor, currentHoverPosition, crossFileDecorations, isCtrlPressed)
              .then((newDecorations) => {
                crossFileDecorations = newDecorations;
              })
              .catch(err => {
                log.error('Error checking cross-file decoration', { error: err });
              });
          }
        }
      }
    };
    
    const globalKeyUpHandler = (e: KeyboardEvent) => {
      if (!e.ctrlKey && !e.metaKey) {
        if (isCtrlPressed) {
          isCtrlPressed = false;

          crossFileDecorations = editor.deltaDecorations(crossFileDecorations, []);
        }
      }
    };
    

    window.addEventListener('keydown', globalKeyDownHandler);
    window.addEventListener('keyup', globalKeyUpHandler);
    

    editor.onMouseMove((e) => {
      const position = e.target.position;
      currentHoverPosition = position;
      
      if (!position) {
        if (crossFileDecorations.length > 0) {
          crossFileDecorations = editor.deltaDecorations(crossFileDecorations, []);
        }
        return;
      }
      
      if (!isCtrlPressed) {

        if (crossFileDecorations.length > 0) {
          crossFileDecorations = editor.deltaDecorations(crossFileDecorations, []);
        }
        return;
      }
      

      this.checkAndShowCrossFileDecoration(editor, position, crossFileDecorations, isCtrlPressed)
        .then((newDecorations) => {
          crossFileDecorations = newDecorations;
        })
        .catch(err => {
          log.error('Error checking cross-file decoration', { error: err });
        });
    });
    

    editor.onMouseLeave(() => {
      currentHoverPosition = null;
      if (crossFileDecorations.length > 0) {
        crossFileDecorations = editor.deltaDecorations(crossFileDecorations, []);
      }
    });
    

    editor.onMouseDown((e) => {
      if (!e.target.position || !isCtrlPressed || !this.pendingCrossFileJump) return;
      
      const position = e.target.position;
      

      if (
        position.lineNumber === this.pendingCrossFileJump.position.lineNumber &&
        position.column >= this.pendingCrossFileJump.position.column - 5 &&
        position.column <= this.pendingCrossFileJump.position.column + 10
      ) {

        e.event.preventDefault();
        e.event.stopPropagation();
        

        this.executeCrossFileJump(this.pendingCrossFileJump);
      }
    });
    

    const gotoDefinitionAction = editor.addAction({
      id: 'editor.action.revealDefinition',
      label: i18nService.t('settings/lsp:editor.goToDefinition'),
      keybindings: [monaco.KeyCode.F12],
      contextMenuGroupId: 'navigation',
      contextMenuOrder: 1.0,
      run: async (editor) => {
        const position = editor.getPosition();
        if (!position) return;
        

        const definition = await this.provideDefinition(editor.getModel()!, position);
        

        if (this.pendingCrossFileJump) {

          this.executeCrossFileJump(this.pendingCrossFileJump);
        } else if (definition && !Array.isArray(definition)) {

          editor.setPosition({
            lineNumber: definition.range.startLineNumber,
            column: definition.range.startColumn
          });
          editor.revealPositionInCenter({
            lineNumber: definition.range.startLineNumber,
            column: definition.range.startColumn
          });
        }
      }
    });
    
    this.disposables.push(gotoDefinitionAction);


    editor.addAction({
      id: 'lsp.findReferences',
      label: i18nService.t('settings/lsp:editor.findAllReferences'),
      keybindings: [
        monaco.KeyMod.Shift | monaco.KeyMod.Alt | monaco.KeyCode.F12
      ],
      contextMenuGroupId: 'navigation',
      contextMenuOrder: 1.5,
      run: async () => {

        editor.trigger('context-menu', 'editor.action.referenceSearch.trigger', null);
      }
    });
    


    const modelDisposeListener = this.model.onWillDispose(() => {
      window.removeEventListener('keydown', globalKeyDownHandler);
      window.removeEventListener('keyup', globalKeyUpHandler);
    });
    

    this.disposables.push(modelDisposeListener);
  }

  
  private refreshInlayHints() {
    try {

      this.editors.forEach(editor => {
        const action = editor.getAction('editor.action.inlayHints.refresh');
        if (action) {
          action.run();
        }
      });
    } catch (error) {
      log.error('Failed to refresh inlay hints', error);
    }
  }

  
  private refreshSemanticTokens() {
    try {

      this.editors.forEach(editor => {
        try {

          const action = editor.getAction('editor.action.semanticTokens.refresh');
          if (action) {
            action.run();
          }
        } catch (_error) {
          // silent
        }
      });
    } catch (error) {
      log.error('Failed to refresh semantic tokens', error);
    }
  }
  
  
  public forceRefreshSemanticTokens() {
    this.refreshSemanticTokens();
  }

  
  private async checkAndShowCrossFileDecoration(
    editor: monaco.editor.IStandaloneCodeEditor,
    position: monaco.Position,
    currentDecorations: string[],
    isCtrlPressed: boolean
  ): Promise<string[]> {
    try {

      if (!isCtrlPressed) {
        return editor.deltaDecorations(currentDecorations, []);
      }
      

      let definition: monaco.languages.Location | monaco.languages.Location[] | null = null;
      try {
        definition = await this.provideDefinition(this.model, position);
      } catch (err) {
        log.error('provideDefinition threw error', { position, error: err });
        return editor.deltaDecorations(currentDecorations, []);
      }
      
      if (!definition) {
        return editor.deltaDecorations(currentDecorations, []);
      }
      
      if (Array.isArray(definition)) {
        return editor.deltaDecorations(currentDecorations, []);
      }
      

      if (this.pendingCrossFileJump) {

        const wordAtPosition = this.model.getWordAtPosition(position);
        
        if (wordAtPosition) {
          const startColumn = wordAtPosition.startColumn;
          const endColumn = wordAtPosition.endColumn;
          

          const newDecorations: monaco.editor.IModelDeltaDecoration[] = [{
            range: new monaco.Range(position.lineNumber, startColumn, position.lineNumber, endColumn),
            options: {
              inlineClassName: 'monaco-cross-file-link',  // CSS class for styling
              hoverMessage: { value: i18nService.t('settings/lsp:editor.goToFile', { fileName: this.pendingCrossFileJump.fileName }) }
            }
          }];
          
          return editor.deltaDecorations(currentDecorations, newDecorations);
        }
      }
      
      return editor.deltaDecorations(currentDecorations, []);
    } catch (error) {
      log.error('Error checking cross-file decoration', { position, error });
      return editor.deltaDecorations(currentDecorations, []);
    }
  }
  
  
  private async executeCrossFileJump(jump: {
    targetPath: string;
    fileName: string;
    targetLine: number;
    targetColumn: number;
  }) {
    const isMarkdownFile = jump.fileName.toLowerCase().endsWith('.md');
    const editorType = isMarkdownFile ? 'markdown-editor' : 'code-editor';
    

    const { normalizePath } = await import('@/shared/utils/pathUtils');
    const normalizedPath = normalizePath(jump.targetPath);
    

    window.dispatchEvent(new CustomEvent('agent-create-tab', {
      detail: {
        type: editorType,
        title: jump.fileName,
        data: {
          filePath: normalizedPath,
          fileName: jump.fileName,
          workspacePath: this.workspaceManager.getWorkspacePath(),
          jumpToLine: jump.targetLine,
          jumpToColumn: jump.targetColumn
        },
        metadata: {
          duplicateCheckKey: normalizedPath
        },
        checkDuplicate: true,
        duplicateCheckKey: normalizedPath,
        replaceExisting: false
      }
    }));
    

    

    this.pendingCrossFileJump = null;
  }

  
  async dispose() {

    const modelUri = this.model.uri.toString();
    GlobalAdapterRegistry.unregister(modelUri);
    

    for (const disposable of this.disposables) {
      disposable.dispose();
    }
    this.disposables = [];
    

    this.completionCache.clear();
  }
  
  
  static enableDiagnosticMode() {
    MonacoLspAdapter.diagnosticMode = true;
    log.info('Diagnostic mode enabled');
  }
  
  
  static disableDiagnosticMode() {
    MonacoLspAdapter.diagnosticMode = false;
    log.info('Diagnostic mode disabled');
  }
  
  
  getDiagnosticInfo() {
    const info = {
      uri: this.uri,
      language: this.language,
      serverLanguage: this.serverLanguage,
      modelUri: this.model.uri.toString(),
      modelLanguage: this.model.getLanguageId(),
      lineCount: this.model.getLineCount(),
      isDisposed: this.disposables.length === 0,
      currentMarkers: monaco.editor.getModelMarkers({ resource: this.model.uri }),
      workspacePath: this.workspaceManager.getWorkspacePath(),
    };
    
    log.debug('Diagnostic info', {
      uri: info.uri,
      language: info.language,
      serverLanguage: info.serverLanguage,
      modelUri: info.modelUri,
      modelLanguage: info.modelLanguage,
      lineCount: info.lineCount,
      isDisposed: info.isDisposed,
      workspacePath: info.workspacePath,
      markerCount: info.currentMarkers.length,
      lspMarkerCount: info.currentMarkers.filter(m => m.source === 'lsp').length
    });
    
    return info;
  }
}
