import React, { useCallback, useEffect, useRef, useState } from 'react';
import { useI18n } from '../i18n';
import { parseScannedPairingLink } from '../services/pairingLink';

interface QrScannerSheetProps {
  onClose: () => void;
  onDetected: (url: string) => void;
}

type ScannerController = {
  start: () => Promise<void>;
  stop: () => void;
  destroy: () => void;
};

function errorName(error: unknown): string {
  if (error instanceof DOMException) return error.name;
  return String((error as { name?: string })?.name || '');
}

const QrScannerSheet: React.FC<QrScannerSheetProps> = ({ onClose, onDetected }) => {
  const { t } = useI18n();
  const videoRef = useRef<HTMLVideoElement>(null);
  const scannerRef = useRef<ScannerController | null>(null);
  const completedRef = useRef(false);
  const [starting, setStarting] = useState(true);
  const [scanningImage, setScanningImage] = useState(false);
  const [manualLink, setManualLink] = useState('');
  const [scanError, setScanError] = useState<string | null>(null);

  const acceptValue = useCallback((rawValue: string) => {
    if (completedRef.current) return;
    const pairingUrl = parseScannedPairingLink(rawValue);
    if (!pairingUrl) {
      setScanError(t('pairing.invalidScannedCode'));
      return;
    }
    completedRef.current = true;
    scannerRef.current?.stop();
    onDetected(pairingUrl);
  }, [onDetected, t]);

  useEffect(() => {
    let disposed = false;
    const startScanner = async () => {
      try {
        const { default: QrScanner } = await import('qr-scanner');
        if (disposed || !videoRef.current) return;
        if (!(await QrScanner.hasCamera())) {
          setScanError(t('pairing.cameraUnavailable'));
          setStarting(false);
          return;
        }
        const scanner = new QrScanner(
          videoRef.current,
          (result) => acceptValue(typeof result === 'string' ? result : result.data),
          {
            preferredCamera: 'environment',
            returnDetailedScanResult: true,
            highlightScanRegion: false,
            highlightCodeOutline: false,
            maxScansPerSecond: 8,
          },
        );
        scannerRef.current = scanner;
        await scanner.start();
        if (!disposed) setStarting(false);
      } catch (error: unknown) {
        if (disposed) return;
        const name = errorName(error);
        setScanError(
          name === 'NotAllowedError' || name === 'PermissionDeniedError'
            ? t('pairing.cameraPermissionDenied')
            : t('pairing.cameraUnavailable'),
        );
        setStarting(false);
      }
    };
    void startScanner();
    return () => {
      disposed = true;
      scannerRef.current?.stop();
      scannerRef.current?.destroy();
      scannerRef.current = null;
    };
  }, [acceptValue, t]);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') onClose();
    };
    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, [onClose]);

  const handleImage = async (event: React.ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    event.target.value = '';
    if (!file) return;
    setScanningImage(true);
    setScanError(null);
    try {
      const { default: QrScanner } = await import('qr-scanner');
      const result = await QrScanner.scanImage(file, { returnDetailedScanResult: true });
      acceptValue(typeof result === 'string' ? result : result.data);
    } catch {
      setScanError(t('pairing.invalidScannedCode'));
    } finally {
      setScanningImage(false);
    }
  };

  return (
    <div className="qr-scanner-sheet__backdrop" role="presentation">
      <section className="qr-scanner-sheet" role="dialog" aria-modal="true" aria-labelledby="qr-scanner-title">
        <header className="qr-scanner-sheet__header">
          <div>
            <h2 id="qr-scanner-title">{t('pairing.scanTitle')}</h2>
            <p>{t('pairing.scanDescription')}</p>
          </div>
          <button type="button" onClick={onClose} aria-label={t('common.close')} autoFocus>
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" aria-hidden="true"><path d="m6 6 12 12M18 6 6 18" /></svg>
          </button>
        </header>

        <div className="qr-scanner-sheet__content">
          <div className="qr-scanner-sheet__camera">
            <video ref={videoRef} muted playsInline aria-label={t('pairing.cameraPreview')} />
            <span className="qr-scanner-sheet__shade" aria-hidden="true" />
            <span className="qr-scanner-sheet__corner qr-scanner-sheet__corner--tl" aria-hidden="true" />
            <span className="qr-scanner-sheet__corner qr-scanner-sheet__corner--tr" aria-hidden="true" />
            <span className="qr-scanner-sheet__corner qr-scanner-sheet__corner--bl" aria-hidden="true" />
            <span className="qr-scanner-sheet__corner qr-scanner-sheet__corner--br" aria-hidden="true" />
            {starting && <span className="qr-scanner-sheet__starting"><span className="spinner" />{t('pairing.scannerStarting')}</span>}
          </div>

          {scanError && <div className="qr-scanner-sheet__error" role="alert">{scanError}</div>}

          <label className="qr-scanner-sheet__image-action">
            <input type="file" accept="image/*" onChange={handleImage} disabled={scanningImage} />
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><rect x="3" y="3" width="18" height="18" rx="3"/><circle cx="9" cy="9" r="2"/><path d="m21 15-4-4L5 21"/></svg>
            {scanningImage ? t('pairing.scanningImage') : t('pairing.scanFromImage')}
          </label>

          <div className="qr-scanner-sheet__manual">
            <label htmlFor="pairing-link-input">{t('pairing.pasteLink')}</label>
            <div>
              <input
                id="pairing-link-input"
                type="url"
                value={manualLink}
                onChange={(event) => setManualLink(event.target.value)}
                placeholder={t('pairing.connectionLinkPlaceholder')}
                autoCapitalize="off"
                autoCorrect="off"
              />
              <button type="button" onClick={() => acceptValue(manualLink)} disabled={!manualLink.trim()}>
                {t('pairing.connectScannedLink')}
              </button>
            </div>
          </div>
        </div>
      </section>
    </div>
  );
};

export default QrScannerSheet;
