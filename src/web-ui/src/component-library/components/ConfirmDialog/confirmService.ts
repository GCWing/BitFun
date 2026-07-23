/**
 * Confirm dialog service
 * Provides imperative APIs for confirmation dialogs.
 */

import { create } from 'zustand';
import type { ConfirmDialogType } from './ConfirmDialog';

export type ConfirmDialogChoice = 'confirm' | 'secondary' | 'cancel';

export interface ConfirmDialogOptions {
  /** Title */
  title: string;
  /** Message content */
  message: React.ReactNode;
  /** Dialog type */
  type?: ConfirmDialogType;
  /** Confirm button text */
  confirmText?: string;
  /** Optional secondary action text */
  secondaryText?: string;
  /** Cancel button text */
  cancelText?: string;
  /** Whether the confirm button uses danger styling */
  confirmDanger?: boolean;
  /** Whether to show the cancel button */
  showCancel?: boolean;
  /** Preview content */
  preview?: string;
  /** Max preview height */
  previewMaxHeight?: number;
}

interface ConfirmDialogState {
  /** Is open */
  isOpen: boolean;
  /** Options */
  options: ConfirmDialogOptions | null;
  /** Resolve callback */
  resolve: ((value: ConfirmDialogChoice) => void) | null;
  
  /** Show the dialog */
  show: (options: ConfirmDialogOptions) => Promise<boolean>;
  /** Show the dialog and return the selected action. */
  showChoice: (options: ConfirmDialogOptions) => Promise<ConfirmDialogChoice>;
  /** Confirm */
  confirm: () => void;
  /** Secondary action */
  secondary: () => void;
  /** Cancel */
  cancel: () => void;
  /** Close */
  close: () => void;
}

export const useConfirmDialogStore = create<ConfirmDialogState>((set, get) => ({
  isOpen: false,
  options: null,
  resolve: null,

  show: (options: ConfirmDialogOptions) => {
    return new Promise<boolean>((resolve) => {
      set({
        isOpen: true,
        options,
        resolve: (value) => resolve(value === 'confirm'),
      });
    });
  },

  showChoice: (options: ConfirmDialogOptions) => {
    return new Promise<ConfirmDialogChoice>((resolve) => {
      set({
        isOpen: true,
        options,
        resolve,
      });
    });
  },

  confirm: () => {
    const { resolve } = get();
    if (resolve) {
      resolve('confirm');
    }
    set({
      isOpen: false,
      options: null,
      resolve: null,
    });
  },

  secondary: () => {
    const { resolve } = get();
    if (resolve) {
      resolve('secondary');
    }
    set({
      isOpen: false,
      options: null,
      resolve: null,
    });
  },

  cancel: () => {
    const { resolve } = get();
    if (resolve) {
      resolve('cancel');
    }
    set({
      isOpen: false,
      options: null,
      resolve: null,
    });
  },

  close: () => {
    const { resolve } = get();
    if (resolve) {
      resolve('cancel');
    }
    set({
      isOpen: false,
      options: null,
      resolve: null,
    });
  },
}));

export function confirmDialog(options: ConfirmDialogOptions): Promise<boolean> {
  return useConfirmDialogStore.getState().show(options);
}

export function confirmDialogChoice(options: ConfirmDialogOptions): Promise<ConfirmDialogChoice> {
  return useConfirmDialogStore.getState().showChoice(options);
}

export function confirmWarning(title: string, message: React.ReactNode, options?: Partial<ConfirmDialogOptions>): Promise<boolean> {
  return confirmDialog({
    title,
    message,
    type: 'warning',
    ...options,
  });
}

export function confirmDanger(title: string, message: React.ReactNode, options?: Partial<ConfirmDialogOptions>): Promise<boolean> {
  return confirmDialog({
    title,
    message,
    type: 'error',
    confirmDanger: true,
    ...options,
  });
}

export function confirmInfo(title: string, message: React.ReactNode, options?: Partial<ConfirmDialogOptions>): Promise<boolean> {
  return confirmDialog({
    title,
    message,
    type: 'info',
    showCancel: false,
    ...options,
  });
}
