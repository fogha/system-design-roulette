// Generated from src-tauri/seed/catalog.json. Run npm run catalog:generate.
export type FocusArea = "system-design" | "javascript" | "typescript" | "frontend-architecture" | "developer-tooling" | "linux-bash" | "bash-scripting";
export type LanguageId = "german" | "italian";
export type ClassroomSubjectId = FocusArea | LanguageId;
export const COURSE_FINGERPRINTS: Record<ClassroomSubjectId, string> = {
  "system-design": "2b6ee917fda6e1b2eccb10a01748f0426e44c8befbe6cbd324646778a1ffc200",
  "javascript": "b03b2c7257dab1f884dbad94f58ae6d1873d0f50a77f900a46cc567d7ae973b2",
  "typescript": "da0ca51fc16b8b1006a5cb7e9e7a24fca5bd1495c0c0731c31e3f15efe3840a7",
  "frontend-architecture": "fbdebc631e57f3669900032e3c4de8f9a07cef86dd1e7a2937616932cd967698",
  "developer-tooling": "0c746bd8a7c7e42883817f5cfda47ed4508baeeeb21a3dcf8fdbd54f090c51de",
  "linux-bash": "4e0138ebf72d918e3b25d4662dba782e77cca08e72835a09392f35e91cd93d5c",
  "bash-scripting": "48c0d497d24e894b3bdb9ce5d066c565d6cd6e31669046bbf76782083f783c38",
  "german": "197b3015b2eb854de871771ecba6ae1de85581c1f0068767d1ab2dda96e99eea",
  "italian": "94114838a3e5959dc11d5315b956b9271709f1c83f1bf06205eef2c2a5fe6080"
};
