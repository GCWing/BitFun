import { QrScanService } from '../../../services/QrScanService';

@ComponentV2
export struct InlineQrScanner {
  private readonly scannerController: XComponentController = new XComponentController();
  private readonly qrScanService: QrScanService = new QrScanService();
  private scannerStarted: boolean = false;
  private scanCompleted: boolean = false;
  private scanStartRetryCount: number = 0;
  private generation: number = 0;
  private cameraPermissionReady: boolean = false;
  @Param scannerSize: number = 248;
  @Param paused: boolean = false;
  @Param restartRevision: number = 0;
  @Event onDetected: (value: string) => void = (_value: string) => {};
  @Event onPermissionDenied: () => void = () => {};
  @Event onUnavailable: () => void = () => {};

  @Monitor('paused', 'restartRevision')
  onScanStateChanged(): void {
    if (this.paused) {
      this.stop();
      return;
    }
    this.scanCompleted = false;
    this.scanStartRetryCount = 0;
    void this.start();
  }

  aboutToDisappear(): void {
    this.stop();
  }

  build() {
    XComponent({ id: 'remote-scan-preview', type: XComponentType.SURFACE, controller: this.scannerController })
      .width(this.scannerSize)
      .height(this.scannerSize)
      .borderRadius(28)
      .onLoad(() => {
        void this.start();
      })
      .onDestroy(() => {
        this.stop();
      })
  }

  private async start(): Promise<void> {
    if (this.paused || this.scannerStarted || this.scanCompleted) {
      return;
    }
    const generation = this.generation;
    if (!this.cameraPermissionReady) {
      const context = this.getUIContext().getHostContext();
      if (!context) {
        this.onPermissionDenied();
        return;
      }
      try {
        await this.qrScanService.ensureCameraPermission(context);
        if (generation !== this.generation || this.paused) {
          return;
        }
        this.cameraPermissionReady = true;
      } catch (_err) {
        this.onPermissionDenied();
        return;
      }
    }

    const surfaceId = this.scannerController.getXComponentSurfaceId();
    if (surfaceId.length === 0) {
      if (this.scanStartRetryCount >= 6) {
        this.onUnavailable();
        return;
      }
      this.scanStartRetryCount += 1;
      setTimeout(() => {
        if (generation === this.generation) {
          void this.start();
        }
      }, 120);
      return;
    }

    this.scanStartRetryCount = 0;
    this.scannerStarted = true;
    this.qrScanService.startInlineScan(surfaceId, (value: string) => {
      if (this.scanCompleted) {
        return;
      }
      this.scanCompleted = true;
      this.stop();
      this.onDetected(value);
    }, (_err: Object) => {
      this.scannerStarted = false;
      this.onUnavailable();
    });
  }

  private stop(): void {
    this.generation += 1;
    this.scannerStarted = false;
    void this.qrScanService.stopInlineScan();
  }
}
