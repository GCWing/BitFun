import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import logoUrl from '../Logo-ICON.png';

interface LanguageSelectProps {
  onSelect: (lang: string) => void;
}

const LANGUAGES = [
  { code: 'en', label: 'English', native: 'English' },
  { code: 'zh', label: 'Chinese', native: '简体中文' },
];

export function LanguageSelect({ onSelect }: LanguageSelectProps) {
  const { i18n } = useTranslation();
  const [selected, setSelected] = useState<string>('en');

  const handleSelect = (code: string) => {
    setSelected(code);
    i18n.changeLanguage(code);
  };

  const handleContinue = () => {
    if (selected) onSelect(selected);
  };

  return (
    <div style={{
      flex: 1, display: 'flex', flexDirection: 'row', overflow: 'hidden',
    }}>
      <div style={{
        flex: '0 0 42%',
        display: 'flex', flexDirection: 'column',
        alignItems: 'center', justifyContent: 'center',
        position: 'relative', overflow: 'hidden',
        background: 'var(--color-bg-primary)',
      }}>
        <div style={{
          position: 'absolute', top: 0, bottom: 0, right: 0, width: 1,
          backgroundImage: `linear-gradient(180deg, var(--border-subtle) 0%, var(--border-subtle) 50%, transparent 50%, transparent 100%)`,
          backgroundSize: '1px 8px', pointerEvents: 'none', zIndex: 10,
        }} />

        <div style={{
          textAlign: 'center', maxWidth: 280, padding: '0 24px',
          animation: 'heroContentFadeIn 0.8s ease-out 0.3s both',
        }}>
          <img src={logoUrl} alt="BitFun" style={{
            display: 'block', margin: '0 auto 16px',
            width: 56, height: 56, borderRadius: 14,
            filter: 'drop-shadow(0 0 40px rgba(100, 180, 255, 0.15))',
          }} />
          <h1 style={{
            fontFamily: 'var(--font-sans)', fontSize: 42, fontWeight: 700,
            color: 'var(--color-text-primary)', letterSpacing: '-0.03em',
            lineHeight: 0.95, margin: '0 0 16px 0',
            textShadow: '0 0 60px rgba(100, 180, 255, 0.3)',
          }}>BitFun</h1>
        </div>

        <div style={{
          position: 'absolute',
          bottom: 24,
          left: 0,
          right: 0,
          zIndex: 2,
          animation: 'fadeIn 1s ease-out 0.8s both',
        }}>
          <div style={{
            maxWidth: 280,
            margin: '0 auto',
            padding: '0 24px',
            textAlign: 'center',
            fontSize: 11,
            color: 'var(--color-text-muted)',
            opacity: 0.6,
            letterSpacing: '0.5px',
          }}>
            Version 0.1.0
          </div>
        </div>
      </div>

      <div style={{
        flex: 1, display: 'flex', flexDirection: 'column',
        alignItems: 'center', justifyContent: 'center',
        padding: '24px 32px',
      }}>
        <div style={{
          display: 'flex', flexDirection: 'column', gap: 16,
          width: '100%', maxWidth: 320,
          animation: 'fadeIn 0.5s ease-out',
        }}>
          <div className="section-label">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
              <circle cx="12" cy="12" r="10" />
              <line x1="2" y1="12" x2="22" y2="12" />
              <path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z" />
            </svg>
            Select Language / 选择语言
          </div>

          <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
            {LANGUAGES.map((lang) => {
              const isSelected = selected === lang.code;
              return (
                <button
                  key={lang.code}
                  onClick={() => handleSelect(lang.code)}
                  style={{
                    display: 'flex', alignItems: 'center', gap: 12,
                    padding: '14px 16px', width: '100%',
                    background: isSelected ? 'rgba(96, 165, 250, 0.14)' : 'rgba(148, 163, 184, 0.08)',
                    border: 'none',
                    borderRadius: 'var(--radius-sm)',
                    cursor: 'pointer', textAlign: 'left',
                    transition: 'all 0.25s ease',
                    outline: 'none',
                    fontFamily: 'var(--font-sans)',
                    boxShadow: 'none',
                  }}
                  onMouseEnter={(e) => {
                    if (!isSelected) {
                      e.currentTarget.style.background = 'rgba(148, 163, 184, 0.14)';
                    }
                  }}
                  onMouseLeave={(e) => {
                    if (!isSelected) {
                      e.currentTarget.style.background = 'rgba(148, 163, 184, 0.08)';
                    }
                  }}
                >
                  <div style={{ flex: 1 }}>
                    <div style={{
                      fontSize: 14, fontWeight: 500,
                      color: isSelected ? 'var(--color-text-primary)' : 'var(--color-text-secondary)',
                      transition: 'color 0.2s ease',
                    }}>{lang.native}</div>
                    <div style={{
                      fontSize: 11,
                      color: 'var(--color-text-muted)', opacity: 0.7,
                      marginTop: 2,
                    }}>{lang.label}</div>
                  </div>
                </button>
              );
            })}
          </div>

          <button
            className={`btn ${selected ? 'btn-primary' : ''}`}
            onClick={handleContinue}
            disabled={!selected}
            style={{
              justifyContent: 'center',
              marginTop: 8,
            }}
          >
            {selected === 'zh' ? '继续' : 'Continue'}
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
              <polyline points="9 18 15 12 9 6" />
            </svg>
          </button>
        </div>
      </div>
    </div>
  );
}
