export type Destination = 'today' | 'classes' | 'schedule' | 'progress' | 'settings';
export const DESTINATIONS: readonly { id: Destination; label: string }[] = [
  { id: 'today', label: 'Today' },
  { id: 'classes', label: 'Classes' },
  { id: 'schedule', label: 'Schedule' },
  { id: 'progress', label: 'Progress' },
  { id: 'settings', label: 'Settings' },
];
