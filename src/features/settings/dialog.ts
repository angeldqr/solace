import { create } from "zustand";

export const useSettingsDialog = create<{ open: boolean }>(() => ({ open: false }));

export const openSettings = () => useSettingsDialog.setState({ open: true });
