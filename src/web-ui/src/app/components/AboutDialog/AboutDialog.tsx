/**
 * About dialog component.
 * Shows app version and license info.
 * Uses component library Modal.
 */

import React, { useState } from 'react';
import { useI18n } from '@/infrastructure/i18n';
import { Tooltip, Modal } from '@/component-library';
import { Copy, Check, Package } from 'lucide-react';
import { systemAPI } from '@/infrastructure/api';
import {
  getAboutInfo,
  formatVersion,
  formatBuildDate
} from '@/shared/utils/version';
import { createLogger } from '@/shared/utils/logger';
import './AboutDialog.scss';

const log = createLogger('AboutDialog');

interface Dependency {
  name: string;
  url: string;
  license: string;
  category: 'frontend' | 'backend';
}

const dependencies: Dependency[] = [
  // Frontend (TypeScript / JS)
  { name: 'React', url: 'https://www.npmjs.com/package/react', license: 'MIT', category: 'frontend' },
  { name: 'React DOM', url: 'https://www.npmjs.com/package/react-dom', license: 'MIT', category: 'frontend' },
  { name: 'Zustand', url: 'https://www.npmjs.com/package/zustand', license: 'MIT', category: 'frontend' },
  { name: 'Immer', url: 'https://www.npmjs.com/package/immer', license: 'MIT', category: 'frontend' },
  { name: 'i18next', url: 'https://www.npmjs.com/package/i18next', license: 'MIT', category: 'frontend' },
  { name: 'react-i18next', url: 'https://www.npmjs.com/package/react-i18next', license: 'MIT', category: 'frontend' },
  { name: 'lucide-react', url: 'https://www.npmjs.com/package/lucide-react', license: 'ISC', category: 'frontend' },
  { name: '@tauri-apps/api', url: 'https://www.npmjs.com/package/@tauri-apps/api', license: 'Apache-2.0', category: 'frontend' },
  { name: '@tauri-apps/plugin-opener', url: 'https://www.npmjs.com/package/@tauri-apps/plugin-opener', license: 'Apache-2.0', category: 'frontend' },
  { name: '@tauri-apps/plugin-dialog', url: 'https://www.npmjs.com/package/@tauri-apps/plugin-dialog', license: 'Apache-2.0', category: 'frontend' },
  { name: '@tanstack/react-virtual', url: 'https://www.npmjs.com/package/@tanstack/react-virtual', license: 'MIT', category: 'frontend' },
  { name: 'Monaco Editor', url: 'https://www.npmjs.com/package/monaco-editor', license: 'MIT', category: 'frontend' },
  { name: '@monaco-editor/react', url: 'https://www.npmjs.com/package/@monaco-editor/react', license: 'MIT', category: 'frontend' },
  { name: 'TipTap', url: 'https://www.npmjs.com/package/@tiptap/react', license: 'MIT', category: 'frontend' },
  { name: 'react-markdown', url: 'https://www.npmjs.com/package/react-markdown', license: 'MIT', category: 'frontend' },
  { name: 'react-syntax-highlighter', url: 'https://www.npmjs.com/package/react-syntax-highlighter', license: 'MIT', category: 'frontend' },
  { name: 'react-virtuoso', url: 'https://www.npmjs.com/package/react-virtuoso', license: 'MIT', category: 'frontend' },
  { name: 'xterm.js', url: 'https://www.npmjs.com/package/@xterm/xterm', license: 'MIT', category: 'frontend' },
  { name: 'Mermaid', url: 'https://www.npmjs.com/package/mermaid', license: 'MIT', category: 'frontend' },
  { name: 'KaTeX', url: 'https://www.npmjs.com/package/katex', license: 'MIT', category: 'frontend' },
  { name: 'highlight.js', url: 'https://www.npmjs.com/package/highlight.js', license: 'BSD-3-Clause', category: 'frontend' },
  { name: 'PrismJS', url: 'https://www.npmjs.com/package/prismjs', license: 'MIT', category: 'frontend' },
  { name: 'diff', url: 'https://www.npmjs.com/package/diff', license: 'BSD-3-Clause', category: 'frontend' },
  { name: 'morphdom', url: 'https://www.npmjs.com/package/morphdom', license: 'MIT', category: 'frontend' },
  { name: 'html-to-image', url: 'https://www.npmjs.com/package/html-to-image', license: 'MIT', category: 'frontend' },
  { name: 'qrcode.react', url: 'https://www.npmjs.com/package/qrcode.react', license: 'MIT', category: 'frontend' },
  { name: 'Vite', url: 'https://www.npmjs.com/package/vite', license: 'MIT', category: 'frontend' },
  { name: 'TypeScript', url: 'https://www.npmjs.com/package/typescript', license: 'Apache-2.0', category: 'frontend' },
  // Backend (Rust)
  { name: 'Tokio', url: 'https://crates.io/crates/tokio', license: 'MIT', category: 'backend' },
  { name: 'Serde', url: 'https://crates.io/crates/serde', license: 'Apache-2.0 OR MIT', category: 'backend' },
  { name: 'Reqwest', url: 'https://crates.io/crates/reqwest', license: 'Apache-2.0 OR MIT', category: 'backend' },
  { name: 'Axum', url: 'https://crates.io/crates/axum', license: 'MIT', category: 'backend' },
  { name: 'Tauri', url: 'https://crates.io/crates/tauri', license: 'Apache-2.0 OR MIT', category: 'backend' },
  { name: 'git2 (libgit2)', url: 'https://crates.io/crates/git2', license: 'Apache-2.0 OR MIT', category: 'backend' },
  { name: 'Chrono', url: 'https://crates.io/crates/chrono', license: 'Apache-2.0 OR MIT', category: 'backend' },
  { name: 'UUID', url: 'https://crates.io/crates/uuid', license: 'Apache-2.0 OR MIT', category: 'backend' },
  { name: 'Regex', url: 'https://crates.io/crates/regex', license: 'Apache-2.0 OR MIT', category: 'backend' },
  { name: 'Anyhow', url: 'https://crates.io/crates/anyhow', license: 'Apache-2.0 OR MIT', category: 'backend' },
  { name: 'Thiserror', url: 'https://crates.io/crates/thiserror', license: 'Apache-2.0 OR MIT', category: 'backend' },
  { name: 'Futures', url: 'https://crates.io/crates/futures', license: 'Apache-2.0 OR MIT', category: 'backend' },
  { name: 'Image', url: 'https://crates.io/crates/image', license: 'Apache-2.0 OR MIT', category: 'backend' },
  { name: 'Zip', url: 'https://crates.io/crates/zip', license: 'MIT', category: 'backend' },
  { name: 'DashMap', url: 'https://crates.io/crates/dashmap', license: 'MIT', category: 'backend' },
  { name: 'IndexMap', url: 'https://crates.io/crates/indexmap', license: 'Apache-2.0 OR MIT', category: 'backend' },
  { name: 'tower-http', url: 'https://crates.io/crates/tower-http', license: 'Apache-2.0 OR MIT', category: 'backend' },
  { name: 'tokio-tungstenite', url: 'https://crates.io/crates/tokio-tungstenite', license: 'MIT', category: 'backend' },
  { name: 'Clap', url: 'https://crates.io/crates/clap', license: 'Apache-2.0 OR MIT', category: 'backend' },
  { name: 'Similar', url: 'https://crates.io/crates/similar', license: 'Apache-2.0 OR MIT', category: 'backend' },
  { name: 'Notifly', url: 'https://crates.io/crates/notify', license: 'Apache-2.0 OR MIT', category: 'backend' },
  { name: 'Fluent', url: 'https://crates.io/crates/fluent-bundle', license: 'Apache-2.0 OR MIT', category: 'backend' },
  { name: 'AES-GCM', url: 'https://crates.io/crates/aes-gcm', license: 'Apache-2.0 OR MIT', category: 'backend' },
  { name: 'X25519-Dalek', url: 'https://crates.io/crates/x25519-dalek', license: 'Apache-2.0 OR MIT', category: 'backend' },
  { name: 'SHA2', url: 'https://crates.io/crates/sha2', license: 'Apache-2.0 OR MIT', category: 'backend' },
  { name: 'russh', url: 'https://crates.io/crates/russh', license: 'MIT', category: 'backend' },
  { name: 'Ratatui', url: 'https://crates.io/crates/ratatui', license: 'MIT', category: 'backend' },
  { name: 'pulldown-cmark', url: 'https://crates.io/crates/pulldown-cmark', license: 'MIT', category: 'backend' },
  { name: 'base64', url: 'https://crates.io/crates/base64', license: 'Apache-2.0 OR MIT', category: 'backend' },
  { name: 'parking_lot', url: 'https://crates.io/crates/parking_lot', license: 'Apache-2.0 OR MIT', category: 'backend' },
];

interface AboutDialogProps {
  /** Whether visible */
  isOpen: boolean;
  /** Close callback */
  onClose: () => void;
}

export const AboutDialog: React.FC<AboutDialogProps> = ({
  isOpen,
  onClose
}) => {
  const { t } = useI18n('common');
  const [copiedItem, setCopiedItem] = useState<string | null>(null);
  const [subDialog, setSubDialog] = useState<'openSource' | 'userAgreement' | null>(null);

  const aboutInfo = getAboutInfo();
  const { version, license } = aboutInfo;

  const copyToClipboard = async (text: string, itemId: string) => {
    try {
      await navigator.clipboard.writeText(text);
      setCopiedItem(itemId);
      setTimeout(() => setCopiedItem(null), 2000);
    } catch (err) {
      log.error('Failed to copy to clipboard', err);
    }
  };

  return (
    <>
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      title={t('header.about')}
      showCloseButton={true}
      size="medium"
    >
      <div className="bitfun-about-dialog__content">
        {/* Hero section - product info */}
        <div className="bitfun-about-dialog__hero">
          <h1 className="bitfun-about-dialog__title">{version.name}</h1>
          <div className="bitfun-about-dialog__version-badge">
            {t('about.version', { version: formatVersion(version.version, version.isDev) })}
          </div>
          <div className="bitfun-about-dialog__divider" />
          <div className="bitfun-about-dialog__dots">
            <span></span>
            <span></span>
            <span></span>
          </div>
        </div>

        {/* Scrollable area */}
        <div className="bitfun-about-dialog__scrollable">
          <div className="bitfun-about-dialog__info-section">
            <div className="bitfun-about-dialog__info-card">
              <div className="bitfun-about-dialog__info-row">
                <span className="bitfun-about-dialog__info-label">{t('about.buildDate')}</span>
                <span className="bitfun-about-dialog__info-value">
                  {formatBuildDate(version.buildDate)}
                </span>
              </div>

              {version.gitCommit && (
                <div className="bitfun-about-dialog__info-row">
                  <span className="bitfun-about-dialog__info-label">{t('about.commit')}</span>
                  <div className="bitfun-about-dialog__info-value-group">
                    <span className="bitfun-about-dialog__info-value bitfun-about-dialog__info-value--mono">
                      {version.gitCommit}
                    </span>
                    <Tooltip content={t('about.copy')}>
                      <button
                        className="bitfun-about-dialog__copy-btn"
                        onClick={() => copyToClipboard(version.gitCommit || '', 'commit')}
                      >
                        {copiedItem === 'commit' ? <Check size={12} /> : <Copy size={12} />}
                      </button>
                    </Tooltip>
                  </div>
                </div>
              )}

              {version.gitBranch && (
                <div className="bitfun-about-dialog__info-row">
                  <span className="bitfun-about-dialog__info-label">{t('about.branch')}</span>
                  <span className="bitfun-about-dialog__info-value">{version.gitBranch}</span>
                </div>
              )}
            </div>
          </div>
        </div>

        {/* Footer */}
        <div className="bitfun-about-dialog__footer">
          <div className="bitfun-about-dialog__links">
            <button
              className="bitfun-about-dialog__link"
              onClick={() => setSubDialog('openSource')}
              type="button"
            >
              {t('about.openSource')}
            </button>
            <span className="bitfun-about-dialog__link-sep">·</span>
            <button
              className="bitfun-about-dialog__link"
              onClick={() => setSubDialog('userAgreement')}
              type="button"
            >
              {t('about.userAgreement')}
            </button>
          </div>
          <p className="bitfun-about-dialog__license">{license.text}</p>
          <p className="bitfun-about-dialog__copyright">
            {t('about.copyright')}
          </p>
        </div>
      </div>
    </Modal>

    {/* Open Source Software dialog */}
    <Modal
      isOpen={subDialog === 'openSource'}
      onClose={() => setSubDialog(null)}
      title={t('about.openSource')}
      showCloseButton={true}
      size="medium"
    >
      <div className="bitfun-about-dialog__sub-content">
        <p className="bitfun-about-dialog__sub-desc">
          {t('about.openSourceDesc')}
        </p>

        <div className="bitfun-about-dialog__dependencies-section">
          <div className="bitfun-about-dialog__sub-category">
            <div className="bitfun-about-dialog__sub-category-header">
              <h3 className="bitfun-about-dialog__sub-category-title">Frontend</h3>
              <span className="bitfun-about-dialog__sub-category-count bitfun-about-dialog__sub-category-count--frontend">
                {dependencies.filter(d => d.category === 'frontend').length}
              </span>
            </div>
            <div className="bitfun-about-dialog__dependencies-grid">
              {dependencies.filter(d => d.category === 'frontend').map((dep) => (
                <div key={dep.name} className="bitfun-about-dialog__dependency-item">
                  <div className="bitfun-about-dialog__dependency-icon">
                    <Package size={12} />
                  </div>
                  <div className="bitfun-about-dialog__dependency-info">
                    <button
                      type="button"
                      className="bitfun-about-dialog__dependency-name"
                      onClick={() => systemAPI.openExternal(dep.url)}
                    >
                      {dep.name}
                    </button>
                    <span className="bitfun-about-dialog__dependency-license">
                      {dep.license}
                    </span>
                  </div>
                  <span className="bitfun-about-dialog__dependency-tag bitfun-about-dialog__dependency-tag--frontend">
                    FE
                  </span>
                </div>
              ))}
            </div>
          </div>
        </div>

        <div className="bitfun-about-dialog__dependencies-section">
          <div className="bitfun-about-dialog__sub-category">
            <div className="bitfun-about-dialog__sub-category-header">
              <h3 className="bitfun-about-dialog__sub-category-title">Backend</h3>
              <span className="bitfun-about-dialog__sub-category-count bitfun-about-dialog__sub-category-count--backend">
                {dependencies.filter(d => d.category === 'backend').length}
              </span>
            </div>
            <div className="bitfun-about-dialog__dependencies-grid">
              {dependencies.filter(d => d.category === 'backend').map((dep) => (
                <div key={dep.name} className="bitfun-about-dialog__dependency-item">
                  <div className="bitfun-about-dialog__dependency-icon">
                    <Package size={12} />
                  </div>
                  <div className="bitfun-about-dialog__dependency-info">
                    <button
                      type="button"
                      className="bitfun-about-dialog__dependency-name"
                      onClick={() => systemAPI.openExternal(dep.url)}
                    >
                      {dep.name}
                    </button>
                    <span className="bitfun-about-dialog__dependency-license">
                      {dep.license}
                    </span>
                  </div>
                  <span className="bitfun-about-dialog__dependency-tag bitfun-about-dialog__dependency-tag--backend">
                    BE
                  </span>
                </div>
              ))}
            </div>
          </div>
        </div>

        <p className="bitfun-about-dialog__sub-footnote">
          {t('about.openSourceFootnote')}
        </p>
      </div>
    </Modal>

    {/* User Agreement dialog */}
    <Modal
      isOpen={subDialog === 'userAgreement'}
      onClose={() => setSubDialog(null)}
      title={t('about.userAgreement')}
      showCloseButton={true}
      size="medium"
    >
      <div className="bitfun-about-dialog__sub-content">
        <section className="bitfun-about-dialog__sub-card">
          <h3 className="bitfun-about-dialog__sub-card-heading">
            1. 服务使用
          </h3>
          <p className="bitfun-about-dialog__sub-card-text">
            用户在使用 BitFun 服务时应遵守相关法律法规。本软件仅供合法用途使用，不得用于任何非法或未经授权的活动。
          </p>
        </section>
        <section className="bitfun-about-dialog__sub-card">
          <h3 className="bitfun-about-dialog__sub-card-heading">
            2. 免责声明
          </h3>
          <p className="bitfun-about-dialog__sub-card-text">
            本软件按"现状"提供，不提供任何明示或暗示的保证。在适用法律允许的最大范围内，开发者不承担任何损害赔偿的责任。使用风险由用户自行承担。
          </p>
        </section>
        <section className="bitfun-about-dialog__sub-card">
          <h3 className="bitfun-about-dialog__sub-card-heading">
            3. 隐私政策
          </h3>
          <p className="bitfun-about-dialog__sub-card-text">
            我们重视你的隐私。本软件可能会收集必要的使用数据以改善服务质量。详细的隐私政策请参阅官方网站。
          </p>
        </section>
        <p className="bitfun-about-dialog__sub-footnote">
          完整协议内容将在后续版本中完善。
        </p>
      </div>
    </Modal>
    </>
  );
};

export default AboutDialog;
