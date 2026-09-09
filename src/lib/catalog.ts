import manifest from '../../src-tauri/seed/catalog.json';
import type { ClassroomSubjectId, FocusArea } from './catalog.generated';

export interface CourseDefinition {
  id: ClassroomSubjectId;
  course_id: string;
  kind: 'engineering' | 'language';
  label: string;
  native_label: string;
  short_code: string;
  title: string;
  summary: string;
  version: string;
  context: string;
  outcome: string;
  environment: string;
  prompt_profile: string;
  prerequisite_courses: readonly ClassroomSubjectId[];
  source_hosts: readonly string[];
  reference_lessons: readonly string[];
  capabilities: readonly string[];
  entry_points: readonly { id: string; label: string }[];
}

// Both native and preview builds derive this metadata from the same manifest.
export const COURSES: readonly CourseDefinition[] = manifest.courses.map(({ prompt_path: _promptPath, ...course }) => ({
  ...course,
  prompt_profile: `classroom.${course.id}`,
})) as CourseDefinition[];

export const ENGINEERING_COURSES = COURSES.filter((course): course is CourseDefinition & { id: FocusArea } => course.kind === 'engineering');

export function courseDefinition(id: string): CourseDefinition | undefined {
  return COURSES.find((course) => course.id === id);
}
