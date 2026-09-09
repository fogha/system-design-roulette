import type { AssessmentResponse, AssessmentRoundId } from './assessments';
import type { CourseReference, EnrollmentDraftId } from './enrollment';
export type AssessmentAttemptId = string & { readonly __entity: 'assessment_attempt' };
export type PlacementVerdict = 'passed' | 'needs_practice' | 'unknown';
export interface CriterionResult {
  id: string; competency: string; entry_point: string; label: string; verdict: PlacementVerdict;
  evidence: { question_id: string; response: AssessmentResponse; verdict: PlacementVerdict; explanation: string; expected_answer: string }[];
}
export interface DiagnosticView {
  attempt_id: AssessmentAttemptId; round_id: AssessmentRoundId; ordinal: number; revision: number;
  questions: { id: string; label: string; prompt: string; choices: { id: string; text: string }[] }[];
  responses: Record<string, AssessmentResponse>; criteria: CriterionResult[];
  submitted: boolean; completed: boolean; can_follow_up: boolean; matches_draft: boolean;
  course: CourseReference; scope_note: string; unknown_areas: string[]; estimated_minutes: number;
}
export interface PathTopic { id: string; label: string; reason: string }
export interface PathRecommendation {
  id: string; draft_id: EnrollmentDraftId; draft_revision: number; course: CourseReference;
  route: 'foundations' | 'manual' | 'diagnostic'; entry_point: string; entry_label: string; explanation: string;
  earlier_topics: PathTopic[]; refreshers: PathTopic[]; criteria: CriterionResult[];
  unknown_areas: string[]; required_outcome: string; assessment_attempt_id: AssessmentAttemptId | null;
}
