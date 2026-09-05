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

const ReleaseLetterModal: React.FC = () => {
  const { t } = useAnnouncementI18n();
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
  const paragraphs = parseReleaseLetterBody(page.body);

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
          <ScrollArea
            className="release-letter-scroll"
            data-openbitfun-component="announcement"
            data-openbitfun-part="releaseLetterScroll"
          >
            <article className="release-letter" aria-labelledby={titleId}>
              <img
                className="release-letter__construction"
                src="/assets/announcements/release-letter-construction.png"
                alt=""
                aria-hidden="true"
                draggable={false}
                data-openbitfun-component="announcement"
                data-openbitfun-part="releaseLetterArtwork"
              />

              <header className="release-letter__header">
                <div className="release-letter__brand">OpenBitFun</div>
                {modal.closable && (
                  <DialogClose
                    className="release-letter__close"
                    aria-label={t('announcements.common.close')}
                  />
                )}
              </header>

              <div className="release-letter__side-word release-letter__side-word--open" aria-hidden="true">
                {'OPEN'.split('').map(letter => <span key={letter}>{letter}</span>)}
              </div>
              <div className="release-letter__side-word release-letter__side-word--bitfun" aria-hidden="true">
                {'BITFUN'.split('').map(letter => <span key={letter}>{letter}</span>)}
              </div>

              <section
                className="release-letter__copy"
                data-openbitfun-component="announcement"
                data-openbitfun-part="releaseLetterCopy"
              >
                <h1 className="release-letter__title" id={titleId}>{page.title}</h1>
                <div className="release-letter__rule" aria-hidden="true" />
                <div className="release-letter__paragraphs" id={descriptionId}>
                  {paragraphs.map((paragraph, index) => (
                    <p key={`${index}-${paragraph.text.slice(0, 20)}`}>
                      {paragraph.emphasized
                        ? <strong>{paragraph.text}</strong>
                        : paragraph.text}
                    </p>
                  ))}
                </div>

                <footer
                  className="release-letter__signature"
                  data-openbitfun-component="announcement"
                  data-openbitfun-part="releaseLetterSignature"
                >
                  <div className="release-letter__team">
                    <span aria-hidden="true" />
                    <p>OpenBitFun Team</p>
                  </div>
                  <img
                    className="release-letter__mascot"
                    src="/assets/announcements/release-letter-mascot.png"
                    alt=""
                    aria-hidden="true"
                    draggable={false}
                  />
                </footer>
              </section>

              <footer
                className="release-letter__marks"
                aria-hidden="true"
                data-openbitfun-component="announcement"
                data-openbitfun-part="releaseLetterMarks"
              >
                <div className="release-letter__version-mark">
                  <span>1.0.0</span>
                  <span>A NEW BEGINNING</span>
                </div>
                <div className="release-letter__build-mark">
                  <span>BUILD MORE</span>
                  <span>TOGETHER</span>
                </div>
              </footer>
            </article>
          </ScrollArea>
        </DialogBody>
      </ThemeRoot>
    </Dialog>
  );
};

export default ReleaseLetterModal;
