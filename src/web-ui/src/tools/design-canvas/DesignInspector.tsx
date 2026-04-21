/**
 * Design Canvas Inspector drawer.
 *
 * Three tabs on a compact right-side drawer inside DesignCanvasPanel:
 * 1. Element — DOM path + computed-style map from the preview iframe.
 * 2. Tokens — CSS custom properties captured from `:root` inside the iframe.
 * 3. Assets — artifact files grouped by kind; clicking opens the file in the Code view.
 */

import React, { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { ClipboardCopy, FileText, Layers, Palette } from 'lucide-react';
import type { DesignArtifactManifest, SelectedElement } from './store/designArtifactStore';
import './DesignInspector.scss';

type InspectorTab = 'element' | 'tokens' | 'assets';

export interface DesignInspectorProps {
  manifest: DesignArtifactManifest;
  selectedElement?: SelectedElement;
  tokens?: Record<string, string>;
  onOpenFile: (path: string) => void;
  onCopyContext: () => void;
}

const EXT_TO_KIND: Record<string, string> = {
  html: 'page',
  css: 'style',
  js: 'script',
  mjs: 'script',
  json: 'data',
  png: 'image',
  jpg: 'image',
  jpeg: 'image',
  webp: 'image',
  svg: 'image',
  ttf: 'font',
  woff: 'font',
  woff2: 'font',
};

const KIND_ORDER = ['page', 'style', 'script', 'data', 'image', 'font', 'other'];

function groupFiles(manifest: DesignArtifactManifest): Record<string, string[]> {
  const groups: Record<string, string[]> = {};
  for (const file of manifest.files) {
    const ext = (file.path.split('.').pop() || '').toLowerCase();
    const kind = EXT_TO_KIND[ext] || 'other';
    if (!groups[kind]) groups[kind] = [];
    groups[kind].push(file.path);
  }
  return groups;
}

export const DesignInspector: React.FC<DesignInspectorProps> = ({
  manifest,
  selectedElement,
  tokens,
  onOpenFile,
  onCopyContext,
}) => {
  const { t } = useTranslation('flow-chat');
  const [tab, setTab] = useState<InspectorTab>('element');

  const grouped = useMemo(() => groupFiles(manifest), [manifest]);
  const sortedGroups = useMemo(() => {
    return Object.entries(grouped).sort(
      (a, b) => KIND_ORDER.indexOf(a[0]) - KIND_ORDER.indexOf(b[0])
    );
  }, [grouped]);

  const tokenEntries = useMemo(() => {
    const source = tokens ?? {};
    return Object.keys(source)
      .sort()
      .map((name) => [name, source[name]] as const);
  }, [tokens]);

  const assetKindLabel = (kind: string) =>
    t(`designCanvas.inspector.assetKind.${kind}`, { defaultValue: kind });

  return (
    <aside className="design-inspector">
      <div className="design-inspector__tabs">
        <button
          type="button"
          className={`design-inspector__tab${tab === 'element' ? ' design-inspector__tab--active' : ''}`}
          onClick={() => setTab('element')}
        >
          <Layers size={13} />
          {t('designCanvas.inspector.tabElement')}
        </button>
        <button
          type="button"
          className={`design-inspector__tab${tab === 'tokens' ? ' design-inspector__tab--active' : ''}`}
          onClick={() => setTab('tokens')}
        >
          <Palette size={13} />
          {t('designCanvas.inspector.tabTokens')}
        </button>
        <button
          type="button"
          className={`design-inspector__tab${tab === 'assets' ? ' design-inspector__tab--active' : ''}`}
          onClick={() => setTab('assets')}
        >
          <FileText size={13} />
          {t('designCanvas.inspector.tabAssets')}
        </button>
      </div>

      <div className="design-inspector__body">
        {tab === 'element' && (
          <div className="design-inspector__section">
            {!selectedElement?.domPath ? (
              <div className="design-inspector__empty">
                {t('designCanvas.inspector.elementHint')}
              </div>
            ) : (
              <>
                <div className="design-inspector__row">
                  <span className="design-inspector__label">{t('designCanvas.inspector.path')}</span>
                  <code className="design-inspector__value">{selectedElement.domPath}</code>
                </div>
                {selectedElement.textExcerpt && (
                  <div className="design-inspector__row">
                    <span className="design-inspector__label">{t('designCanvas.inspector.text')}</span>
                    <span className="design-inspector__value">
                      “{selectedElement.textExcerpt}”
                    </span>
                  </div>
                )}
                {selectedElement.rect && (
                  <div className="design-inspector__row">
                    <span className="design-inspector__label">{t('designCanvas.inspector.box')}</span>
                    <code className="design-inspector__value">
                      {`${Math.round(selectedElement.rect.width)}×${Math.round(
                        selectedElement.rect.height
                      )} @ (${Math.round(selectedElement.rect.x)}, ${Math.round(
                        selectedElement.rect.y
                      )})`}
                    </code>
                  </div>
                )}
                <div className="design-inspector__subhead">{t('designCanvas.inspector.computedStyles')}</div>
                <div className="design-inspector__styles">
                  {selectedElement.computedStyle &&
                  Object.keys(selectedElement.computedStyle).length > 0 ? (
                    Object.entries(selectedElement.computedStyle).map(([name, value]) => (
                      <div key={name} className="design-inspector__style">
                        <code className="design-inspector__style-name">{name}</code>
                        <span className="design-inspector__style-value">{value}</span>
                      </div>
                    ))
                  ) : (
                    <div className="design-inspector__empty">{t('designCanvas.inspector.noStyleData')}</div>
                  )}
                </div>
                <button
                  type="button"
                  className="design-inspector__copy-btn"
                  onClick={onCopyContext}
                >
                  <ClipboardCopy size={12} />
                  {t('designCanvas.inspector.copyContext')}
                </button>
              </>
            )}
          </div>
        )}

        {tab === 'tokens' && (
          <div className="design-inspector__section">
            <div className="design-inspector__subhead">{t('designCanvas.inspector.tokensHead')}</div>
            {tokenEntries.length === 0 ? (
              <div className="design-inspector__empty">
                {t('designCanvas.inspector.noCssVariables')}
              </div>
            ) : (
              <div className="design-inspector__tokens">
                {tokenEntries.map(([name, value]) => (
                  <div key={name} className="design-inspector__token">
                    <span
                      className="design-inspector__token-swatch"
                      style={{
                        background:
                          /^#|^rgb|^hsl|^oklch/.test(value) || /color/i.test(name)
                            ? value
                            : 'transparent',
                      }}
                    />
                    <code className="design-inspector__token-name">{name}</code>
                    <span className="design-inspector__token-value">{value}</span>
                  </div>
                ))}
              </div>
            )}
          </div>
        )}

        {tab === 'assets' && (
          <div className="design-inspector__section">
            {sortedGroups.map(([kindKey, files]) => (
              <div key={kindKey} className="design-inspector__asset-group">
                <div className="design-inspector__subhead">{assetKindLabel(kindKey)}</div>
                <ul className="design-inspector__asset-list">
                  {files.map((path) => (
                    <li key={path}>
                      <button
                        type="button"
                        className="design-inspector__asset-item"
                        onClick={() => onOpenFile(path)}
                        title={path}
                      >
                        {path}
                      </button>
                    </li>
                  ))}
                </ul>
              </div>
            ))}
          </div>
        )}
      </div>
    </aside>
  );
};

export default DesignInspector;
