import { SelectedImageAttachment } from '../model/RemoteModels';
import { AppRoute, AppRouteContract } from '../pages/navigation/AppRouteContract';
import { GeneralChatPageState } from '../pages/state/GeneralChatPageState';
import { RemotePageState } from '../pages/state/RemotePageState';
import { VoiceInputRouteSnapshot } from './VoiceInputLifecycleController';

/** Keeps route-dependent composer state mapping out of the root component. */
export class AppRootRouteState {
  static isGeneralComposerRoute(route: AppRoute): boolean {
    return AppRouteContract.isGeneralComposerRoute(route);
  }

  static chatInput(route: AppRoute, general: GeneralChatPageState, remote: RemotePageState): string {
    return AppRootRouteState.isGeneralComposerRoute(route) ? general.chatInput : remote.chatInput;
  }

  static selectedImages(route: AppRoute, general: GeneralChatPageState,
    remote: RemotePageState): SelectedImageAttachment[] {
    return AppRootRouteState.isGeneralComposerRoute(route) ? general.selectedImages : remote.selectedImages;
  }

  static voiceListening(route: AppRoute, general: GeneralChatPageState, remote: RemotePageState): boolean {
    return AppRootRouteState.isGeneralComposerRoute(route) ? general.isVoiceListening : remote.isVoiceListening;
  }

  static setChatInput(route: AppRoute, value: string, general: GeneralChatPageState, remote: RemotePageState): void {
    if (AppRootRouteState.isGeneralComposerRoute(route)) {
      general.setChatInput(value);
      return;
    }
    remote.setChatInput(value);
  }

  static setSelectedImages(route: AppRoute, images: SelectedImageAttachment[], general: GeneralChatPageState,
    remote: RemotePageState): void {
    if (AppRootRouteState.isGeneralComposerRoute(route)) {
      general.setSelectedImages(images);
      return;
    }
    remote.setSelectedImages(images);
  }

  static addSelectedImages(route: AppRoute, images: SelectedImageAttachment[], general: GeneralChatPageState,
    remote: RemotePageState): void {
    if (AppRootRouteState.isGeneralComposerRoute(route)) {
      general.addSelectedImages(images);
      return;
    }
    remote.addSelectedImages(images);
  }

  static removeSelectedImage(route: AppRoute, imageId: string, general: GeneralChatPageState,
    remote: RemotePageState): void {
    if (AppRootRouteState.isGeneralComposerRoute(route)) {
      general.removeSelectedImage(imageId);
      return;
    }
    remote.removeSelectedImage(imageId);
  }

  static clearComposer(route: AppRoute, general: GeneralChatPageState, remote: RemotePageState): void {
    if (AppRootRouteState.isGeneralComposerRoute(route)) {
      general.clearComposer();
      return;
    }
    remote.clearComposer();
  }

  static setVoiceListening(route: AppRoute, isListening: boolean, general: GeneralChatPageState,
    remote: RemotePageState): void {
    if (AppRootRouteState.isGeneralComposerRoute(route)) {
      general.setVoiceListening(isListening);
      return;
    }
    remote.setVoiceListening(isListening);
  }

  static snapshot(route: AppRoute, isBusy: boolean, general: GeneralChatPageState,
    remote: RemotePageState): VoiceInputRouteSnapshot {
    return {
      routeId: `${route}`,
      isListening: AppRootRouteState.voiceListening(route, general, remote),
      isBusy,
      inputText: AppRootRouteState.chatInput(route, general, remote),
      selectedImageCount: AppRootRouteState.selectedImages(route, general, remote).length
    };
  }
}
