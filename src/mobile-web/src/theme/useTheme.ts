import { useContext } from 'react';
import { ThemeContext } from './ThemeProvider';

export function useTheme() {
  return useContext(ThemeContext);
}
