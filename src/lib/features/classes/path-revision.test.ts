import { beforeEach, describe, expect, it } from 'vitest';
import { mockApi } from '../../mock';
import { previewClassPath } from '../../class-preview';

async function acceptFoundations(courseId: 'typescript') {
  const options = await mockApi.getEnrollmentOptions(courseId);
  const draft = await mockApi.saveEnrollmentDraft({ id: null, expected_revision: null, course: options.course, configuration: options.default_configuration });
  const recommendation = await mockApi.getPathRecommendation(draft.id, draft.revision);
  return mockApi.acceptClassPath({ draft_id: draft.id, expected_revision: draft.revision, recommendation_id: recommendation.id });
}

describe('path revisions in the browser preview', () => {
  beforeEach(() => { if (typeof localStorage !== 'undefined') localStorage.clear(); });

  it('bypasses and includes topics as new revisions without credit', async () => {
    const accepted = await acceptFoundations('typescript');
    const map = await mockApi.getCurriculumMap('typescript');
    const topic = map.concepts.find((concept) => concept.core)!;
    expect(map.path).toMatchObject({ revision: accepted.revision, required_done: 0, bypassed: 0 });
    const revised = await mockApi.reviseClassPath({ course_id: 'typescript', expected_revision: accepted.revision, change: { kind: 'bypass', topics: [topic.slug] } });
    expect(revised.revision).toBe(accepted.revision + 1);
    expect(revised.reference.class_id).toBe(accepted.reference.class_id);
    expect(revised.recommendation.bypassed?.map((t) => t.id)).toEqual([topic.slug]);
    const after = await mockApi.getCurriculumMap('typescript');
    expect(after.concepts.find((concept) => concept.slug === topic.slug)).toMatchObject({ path_status: 'bypassed_by_choice', required: false });
    expect(after.path?.required_total).toBe(map.path!.required_total - 1);
    expect(after.path?.coverage_total).toBe(map.path!.coverage_total);
    await expect(mockApi.reviseClassPath({ course_id: 'typescript', expected_revision: accepted.revision, change: { kind: 'bypass', topics: [topic.slug] } })).rejects.toThrow('path changed');
    const included = await mockApi.reviseClassPath({ course_id: 'typescript', expected_revision: revised.revision, change: { kind: 'include', topics: [topic.slug] } });
    expect(included.revision).toBe(revised.revision + 1);
    const latest = previewClassPath('typescript')!.recommendation;
    expect(latest.earlier_topics.some((t) => t.id === topic.slug)).toBe(false);
    expect(latest.bypassed).toEqual([]);
    expect((await mockApi.getCurriculumMap('typescript')).concepts.find((concept) => concept.slug === topic.slug)?.path_status).toBe('upcoming');
    await expect(mockApi.reviseClassPath({ course_id: 'typescript', expected_revision: included.revision, change: { kind: 'include', topics: [topic.slug] } })).rejects.toThrow('as it is');
  });
});

describe('bridge decisions in the browser preview', () => {
  beforeEach(() => { if (typeof localStorage !== 'undefined') localStorage.clear(); });

  it('records accepted and declined bridges as revisions', async () => {
    const accepted = await acceptFoundations('typescript');
    const map = await mockApi.getCurriculumMap('typescript');
    const dependent = map.concepts.find((concept) => concept.prerequisites.length)!;
    const prerequisite = dependent.prerequisites[0];
    const declined = await mockApi.reviseClassPath({ course_id: 'typescript', expected_revision: accepted.revision, change: { kind: 'decline_bridge', topic: prerequisite, before: dependent.slug } });
    expect(declined.recommendation.declined_bridges).toEqual([{ topic: prerequisite, before: dependent.slug }]);
    await expect(mockApi.reviseClassPath({ course_id: 'typescript', expected_revision: declined.revision, change: { kind: 'decline_bridge', topic: prerequisite, before: dependent.slug } })).rejects.toThrow('as it is');
    const bridged = await mockApi.reviseClassPath({ course_id: 'typescript', expected_revision: declined.revision, change: { kind: 'accept_bridge', topic: prerequisite, before: dependent.slug } });
    expect(bridged.recommendation.bridges?.map((t) => t.id)).toEqual([prerequisite]);
    expect(bridged.recommendation.declined_bridges).toEqual([]);
    const after = await mockApi.getCurriculumMap('typescript');
    expect(after.concepts.find((concept) => concept.slug === prerequisite)?.path_status).toBe('bridge');
    expect(after.path?.bridges).toBe(1);
  });
});
