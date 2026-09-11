// Generated from src-tauri/seed/catalog.json. Run npm run catalog:generate.
export type FocusArea = "system-design" | "javascript" | "typescript" | "frontend-architecture" | "developer-tooling" | "linux-bash" | "bash-scripting";
export type LanguageId = "german" | "italian";
export type ClassroomSubjectId = FocusArea | LanguageId;
export const COURSE_FINGERPRINTS: Record<ClassroomSubjectId, string> = {
  "system-design": "6553605e71097004df1a41aa2705f297f00bb7cfe87200230437ff4221b94203",
  "javascript": "b804c653fcc839e1c34b000e91404ed43011d6584dee310ce27bc467ded0da54",
  "typescript": "379763f7182cc4cecd5f12c89f4fda45cf60559c8cb29fe8a0a9448b5ae41c16",
  "frontend-architecture": "dbfb6d03b5288a8ab36e0109ebb4b40e6aa916af6960a3c783d9a487cb98b0f1",
  "developer-tooling": "c931434e7da0fb8c12f573e8483d98d32e6f4f39a5a6001ebbee72a238b4fbcf",
  "linux-bash": "4e0138ebf72d918e3b25d4662dba782e77cca08e72835a09392f35e91cd93d5c",
  "bash-scripting": "48c0d497d24e894b3bdb9ce5d066c565d6cd6e31669046bbf76782083f783c38",
  "german": "197b3015b2eb854de871771ecba6ae1de85581c1f0068767d1ab2dda96e99eea",
  "italian": "94114838a3e5959dc11d5315b956b9271709f1c83f1bf06205eef2c2a5fe6080"
};
