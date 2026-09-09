import type { AcceptedPath, AcceptPath, PathSummary } from './contracts/classes';
import type { ClassroomSubjectId } from './catalog.generated';
import { findPreviewEnrollmentDraft } from './enrollment-preview';
import { recommendPreviewPath } from './placement-preview';
import { previewClassRecords, savePreviewClassRecords } from './class-preview-store';

export function previewClassPath(courseId: ClassroomSubjectId): AcceptedPath | null {
  return previewClassRecords().filter(r => r.path.recommendation.course.course_id === courseId).at(-1)?.path ?? null;
}
export function previewPathSummary(path: AcceptedPath | null): PathSummary | null {
  if (!path) return null;
  return { ...path.reference, revision: path.revision, entry_point: path.recommendation.entry_point, entry_label: path.recommendation.entry_label, route: path.recommendation.route, earlier_topics: path.recommendation.earlier_topics.length, refreshers: path.recommendation.refreshers.length };
}
export async function acceptPreviewClassPath(input: AcceptPath): Promise<AcceptedPath> {
  // Recommendation evaluation is asynchronous; recheck acceptance on both sides
  // so concurrent clicks cannot create two revisions for the same draft.
  const existing = previewClassRecords().find(r => r.draft.id === input.draft_id);
  if (existing) {
    if (existing.path.recommendation.id !== input.recommendation_id || existing.draft.revision !== input.expected_revision) throw new Error('This draft was accepted with a different recommendation.');
    return existing.path;
  }
  const recommendation = await recommendPreviewPath(input.draft_id, input.expected_revision);
  if (previewClassRecords().some(r => r.draft.id === input.draft_id)) return acceptPreviewClassPath(input);
  if (recommendation.id !== input.recommendation_id) throw new Error('The recommendation changed. Review the current path before accepting it.');
  const draft = findPreviewEnrollmentDraft(input.draft_id)!;
  if (draft.configuration.focus_policy !== 'advisory') throw new Error('Class paths currently support advisory focus. Choose advisory before accepting.');
  const previous = previewClassPath(draft.course.course_id);
  const path: AcceptedPath = {
    reference: { class_id: previous?.reference.class_id ?? `preview-class-${crypto.randomUUID()}`, path_revision_id: `preview-path-${crypto.randomUUID()}`, course_snapshot_fingerprint: draft.course.fingerprint },
    revision: (previous?.revision ?? 0) + 1, configuration: structuredClone(draft.configuration), recommendation, accepted_at: new Date().toISOString(),
  };
  savePreviewClassRecords([...previewClassRecords(), { path, draft: { ...draft, status: 'accepted', accepted_class_id: path.reference.class_id, updated_at: path.accepted_at } }]);
  return path;
}
