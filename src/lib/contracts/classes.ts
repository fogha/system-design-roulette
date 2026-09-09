import type { EnrollmentConfiguration, EnrollmentDraftId } from './enrollment';
import type { PathRecommendation } from './placement';
export interface PathReference { class_id: string; path_revision_id: string; course_snapshot_fingerprint: string }
export interface AcceptedPath { reference: PathReference; revision: number; configuration: EnrollmentConfiguration; recommendation: PathRecommendation; accepted_at: string }
export interface PathSummary { class_id: string; path_revision_id: string; revision: number; entry_point: string; entry_label: string; route: string; earlier_topics: number; refreshers: number }
export interface AcceptPath { draft_id: EnrollmentDraftId; expected_revision: number; recommendation_id: string }
