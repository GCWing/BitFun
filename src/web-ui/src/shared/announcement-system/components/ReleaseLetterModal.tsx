import React, { useCallback, useId, useRef } from 'react';
import {
  Dialog,
  DialogBody,
  DialogClose,
  ScrollArea,
  ThemeRoot,
} from '@openbitfun/ui';
import { useAnnouncementI18n } from '../hooks/useAnnouncementI18n';
import { useAnnouncementStore } from '../store/announcementStore';
import { useReleaseLetterMotion } from '../hooks/useReleaseLetterMotion';
import ReleaseLetterDrawing from './ReleaseLetterDrawing';
import ReleaseLetterMascot from './ReleaseLetterMascot';
import { SIGNATURE } from './releaseLetterMotion';
import '../styles/ReleaseLetterModal.scss';

interface ReleaseLetterParagraph {
  emphasized: boolean;
  text: string;
}

export function parseReleaseLetterBody(body: string): ReleaseLetterParagraph[] {
  return body
    .split(/\r?\n\s*\r?\n/)
    .map(paragraph => paragraph.trim())
    .filter(Boolean)
    .map((paragraph) => {
      const emphasis = paragraph.match(/^\*\*(.+)\*\*$/s);
      return {
        emphasized: Boolean(emphasis),
        text: emphasis?.[1] ?? paragraph,
      };
    });
}

function ReleaseLetterScene({ title, body, titleId, descriptionId, closable }: {
  title: string; body: string; titleId: string; descriptionId: string; closable: boolean;
}) {
  const { t, currentLanguage } = useAnnouncementI18n();
  const { sceneRef, replay, skip, bounce } = useReleaseLetterMotion();
  const paragraphs = parseReleaseLetterBody(body);
  return (
    <div ref={sceneRef} className="release-letter-scene" data-motion-state="intro" data-content-ready="false" lang={currentLanguage}>
      <ScrollArea className="release-letter-scroll" data-openbitfun-component="announcement" data-openbitfun-part="releaseLetterScroll">
        <article className="release-letter" aria-labelledby={titleId}>
          <div className="release-letter__construction" aria-hidden="true" data-openbitfun-component="announcement" data-openbitfun-part="releaseLetterArtwork">
            <div className="release-letter__drawing-box">
              <ReleaseLetterDrawing />
              <div className="release-letter__intro-wordmark">OpenBitFun</div>
              <div className="release-letter__intro-rule" />
            </div>
          </div>
          <header className="release-letter__header">
            <div className="release-letter__brand" data-reveal="0">OpenBitFun</div>
          </header>
          <section className="release-letter__copy" data-openbitfun-component="announcement" data-openbitfun-part="releaseLetterCopy">
            <h1 className="release-letter__title" id={titleId} data-reveal="1">{title}</h1>
            <div className="release-letter__rule" aria-hidden="true" data-reveal="2" />
            <div className="release-letter__paragraphs" id={descriptionId}>
              {paragraphs.map((paragraph, index) => (
                <p key={`${index}-${paragraph.text.slice(0, 20)}`} data-reveal={index + 3}>
                  {paragraph.emphasized ? <strong>{paragraph.text}</strong> : paragraph.text}
                </p>
              ))}
            </div>
            <footer className="release-letter__signature" data-openbitfun-component="announcement" data-openbitfun-part="releaseLetterSignature">
              <div className="release-letter__team">
                <span aria-hidden="true" />
                <p>
                  <span className="release-letter__sr-only">{SIGNATURE}</span>
                  <span aria-hidden="true">{Array.from(SIGNATURE).map((character, index) => (
                    <span key={index} className="release-letter__typing-char" data-typing-char="" data-typed="false">{character}</span>
                  ))}</span>
                </p>
              </div>
              <button type="button" className="release-letter__mascot-button" onPointerEnter={bounce} onFocus={bounce} onClick={bounce} aria-label={t('announcements.release_letter.replay_mascot')}>
                <ReleaseLetterMascot />
              </button>
            </footer>
          </section>
          <footer className="release-letter__marks" data-reveal="10" data-openbitfun-component="announcement" data-openbitfun-part="releaseLetterMarks">
            <button type="button" className="release-letter__version-mark" onClick={replay} aria-label={t('announcements.release_letter.replay')}>
              <span>1.0.0</span><span>A NEW BEGINNING</span>
            </button>
            <div className="release-letter__build-mark" aria-hidden="true"><span>BUILD MORE</span><span>TOGETHER</span></div>
          </footer>
        </article>
      </ScrollArea>
      <div className="release-letter__controls">
        <button type="button" className="release-letter__skip" onClick={skip}>{t('announcements.release_letter.skip')}</button>
        {closable && <DialogClose className="release-letter__close" aria-label={t('announcements.common.close')} />}
      </div>
    </div>
  );
}

const ReleaseLetterModal: React.FC = () => {
  const {
    closeModal,
    markModalPresented,
    modalVisible,
    openModal,
  } = useAnnouncementStore();
  const titleId = useId();
  const descriptionId = useId();
  const mountedCardIdRef = useRef<string | null>(null);

  const setSurfaceRef = useCallback((node: HTMLDivElement | null) => {
    if (
      node
      && modalVisible
      && openModal
      && mountedCardIdRef.current !== openModal.id
    ) {
      mountedCardIdRef.current = openModal.id;
      markModalPresented(openModal);
    }
  }, [markModalPresented, modalVisible, openModal]);

  if (!openModal || openModal.modal?.presentation !== 'release_letter') return null;

  const modal = openModal.modal;
  const page = modal.pages[0];
  if (!page) return null;

  return (
    <Dialog
      ref={setSurfaceRef}
      className="release-letter-dialog"
      open={modalVisible}
      onOpenChange={(nextOpen) => {
        if (!nextOpen && modalVisible) closeModal();
      }}
      closeOnEscape={modal.closable}
      closeOnPointerOutside={modal.closable}
      size="2xl"
      aria-labelledby={titleId}
      aria-describedby={descriptionId}
    >
      <ThemeRoot
        className="release-letter-theme"
        colorScheme="light"
        density="comfortable"
        data-openbitfun-component="announcement"
        data-openbitfun-part="releaseLetter"
      >
        <DialogBody className="release-letter-dialog__body" inset="none">
          {modalVisible && <ReleaseLetterScene key={openModal.id} title={page.title} body={page.body} titleId={titleId} descriptionId={descriptionId} closable={modal.closable} />}
        </DialogBody>
      </ThemeRoot>
    </Dialog>
  );
};

export default ReleaseLetterModal;
