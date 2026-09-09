// Generated from src-tauri/seed/catalog.json. Run npm run catalog:generate.
export type FocusArea = "system-design" | "javascript" | "typescript" | "frontend-architecture" | "developer-tooling" | "linux-bash" | "bash-scripting";
export type LanguageId = "german" | "italian";
export type ClassroomSubjectId = FocusArea | LanguageId;
export const COURSE_FINGERPRINTS: Record<ClassroomSubjectId, string> = {
  "system-design": "6553605e71097004df1a41aa2705f297f00bb7cfe87200230437ff4221b94203",
  "javascript": "19eb505d984b11914f5edf6718f7d463b5e411dc14ff1b4b8069a57a1c2881e8",
  "typescript": "ea7143364a79a036b632ab134b3c61810d079a1bd2abe5b42a32def51e7cd9c7",
  "frontend-architecture": "dbfb6d03b5288a8ab36e0109ebb4b40e6aa916af6960a3c783d9a487cb98b0f1",
  "developer-tooling": "c931434e7da0fb8c12f573e8483d98d32e6f4f39a5a6001ebbee72a238b4fbcf",
  "linux-bash": "a3e83d73165392fd341de26f57e472724a7ae0425848a82b47af48629775a133",
  "bash-scripting": "dbb85b20c117e3bc9cc9af372651b62838f58a9ff0963901d84c523c7ac5c726",
  "german": "197b3015b2eb854de871771ecba6ae1de85581c1f0068767d1ab2dda96e99eea",
  "italian": "94114838a3e5959dc11d5315b956b9271709f1c83f1bf06205eef2c2a5fe6080"
};
