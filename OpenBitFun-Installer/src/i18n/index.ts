import i18n from 'i18next';
import { initReactI18next } from 'react-i18next';
import {
  DEFAULT_INSTALLER_UI_LANGUAGE,
  getInstallerUiFallbackChain,
  installerResources,
} from './languages';

i18n.use(initReactI18next).init({
  resources: installerResources,
  lng: DEFAULT_INSTALLER_UI_LANGUAGE,
  fallbackLng: (code) => getInstallerUiFallbackChain(code),
  interpolation: { escapeValue: false },
});

export default i18n;
