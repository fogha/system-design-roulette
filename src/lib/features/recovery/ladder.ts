/**
 * The recovery ladder, as the desk documents it everywhere: the key
 * combination, the four console commands, and what stands behind them.
 * The Rust side holds the same text; this copy renders before it answers.
 */
export const RECOVERY_SHORTCUT = 'Ctrl + Alt + Shift + U';
export const RECOVERY_SHORTCUT_MAC = 'Control + Option + Shift + U';
export const VALVE_PRESSES = 5;
export const VALVE_SECONDS = 10;

export const LADDER: { command: string; what: string }[] = [
  { command: 'unlock', what: 'Explains what will happen and issues a six-character challenge code.' },
  { command: 'confirm <code>', what: 'Type the code back. It proves a person is at the keyboard.' },
  { command: 'phrase <your escape phrase>', what: 'The break-glass phrase you set during setup.' },
  { command: 'release', what: 'Pauses the session with its work intact, breaks the streak, and frees the machine.' },
];

/** The combination as it reads on this machine. */
export function shortcutLabel(): string {
  const mac = typeof navigator !== 'undefined' && /Mac|iPhone|iPad/.test(navigator.platform ?? navigator.userAgent);
  return mac ? RECOVERY_SHORTCUT_MAC : RECOVERY_SHORTCUT;
}
