import React, { useCallback, useEffect, useRef, useState } from 'react';
import { MobileBanner, MobileButton, MobileFileButton, MobileIconButton, MobileSheet, MobileTextField } from '@openbitfun/ui/mobile';
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
    <MobileSheet
      className="qr-scanner-sheet"
      description={t('pairing.scanDescription')}
      headerAction={<MobileIconButton appearance="plain" aria-label={t('common.close')} autoFocus icon={<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" aria-hidden="true"><path d="m6 6 12 12M18 6 6 18" /></svg>} onClick={onClose} size="sm" />}
      onOpenChange={onClose}
      open
      showHandle={false}
      title={t('pairing.scanTitle')}
    >
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

          {scanError && <MobileBanner className="qr-scanner-sheet__error" tone="danger">{scanError}</MobileBanner>}

          <MobileFileButton
            accept="image/*"
            className="qr-scanner-sheet__image-action"
            leading={<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><rect x="3" y="3" width="18" height="18" rx="3"/><circle cx="9" cy="9" r="2"/><path d="m21 15-4-4L5 21"/></svg>}
            loading={scanningImage}
            onChange={handleImage}
          >
            {scanningImage ? t('pairing.scanningImage') : t('pairing.scanFromImage')}
          </MobileFileButton>

          <div className="qr-scanner-sheet__manual">
            <label htmlFor="pairing-link-input">{t('pairing.pasteLink')}</label>
            <div>
              <MobileTextField
                appearance="surface"
                id="pairing-link-input"
                type="url"
                value={manualLink}
                onChange={(event) => setManualLink(event.target.value)}
                placeholder={t('pairing.connectionLinkPlaceholder')}
                autoCapitalize="off"
                autoCorrect="off"
              />
              <MobileButton onClick={() => acceptValue(manualLink)} disabled={!manualLink.trim()}>
                {t('pairing.connectScannedLink')}
              </MobileButton>
            </div>
          </div>
        </div>
    </MobileSheet>
  );
};

export default QrScannerSheet;
