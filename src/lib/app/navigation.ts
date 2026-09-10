export type Destination = 'today' | 'classes' | 'progress' | 'settings';
export const DESTINATIONS: readonly { id: Destination; label: string }[] = [
  { id: 'today', label: 'Today' },
  { id: 'classes', label: 'Classes' },
  { id: 'progress', label: 'Progress' },
  { id: 'settings', label: 'Settings' },
];
