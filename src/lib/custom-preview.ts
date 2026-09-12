/**
 * The class builder in the browser preview: an in-memory copy of what the
 * desk stores, a canned draft standing in for the tutor, and the same
 * validator rules the native side holds a draft to, so the editor behaves
 * the same way before the desktop app is attached.
 */
import type { ClassExport, CourseBrief, CourseDraft, CustomCourseSummary, CustomCourseView, DraftIssue, DraftTopic, QuestionBank, ReviewFinding, SourceCheck } from './ipc';
import { registerCourses, type CourseDefinition } from './catalog';

export const STAGES: { id: DraftTopic['curriculum']['phase']; label: string }[] = [
  { id: 'foundations', label: 'Foundations' },
  { id: 'mechanisms', label: 'Core mechanisms' },
  { id: 'production', label: 'Practical application' },
  { id: 'synthesis', label: 'Integration and capstone' },
];
export const MIN_TOPICS = 6;
export const MAX_TOPICS = 60;

const tier = (phase: string) => ({ foundations: 0, mechanisms: 1, production: 2 }[phase] ?? 3);
const words = (text: string) => text.trim().split(/\s+/).filter(Boolean).length;
const hostOf = (url: string) => { try { return new URL(url).hostname.toLowerCase(); } catch { return null; } };
const onHost = (hosts: string[], url: string) => { const host = hostOf(url); return !!host && hosts.some((h) => host === h || host.endsWith(`.${h}`)); };

export function slugify(text: string): string {
  return text.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-+|-+$/g, '').slice(0, 48);
}

/** Mirror of `domain::custom::validate`; the native result stays authoritative. */
export function validateDraft(draft: CourseDraft): DraftIssue[] {
  const issues: DraftIssue[] = [];
  const issue = (at: string, message: string) => issues.push({ at, message });
  if (!draft.label.trim()) issue('label', 'give the class a name');
  if (!draft.short_code.trim() || draft.short_code.trim().length > 4) issue('short_code', 'a short code is one to four characters');
  if (words(draft.summary) < 6) issue('summary', 'the summary needs a sentence');
  if (words(draft.outcome) < 15) issue('outcome', 'the outcome names what you will be able to make or do at the end; say it in at least fifteen words');
  if (words(draft.context) < 15) issue('context', 'the context tells the tutor how this subject should be taught; at least fifteen words');
  if (words(draft.environment) < 3) issue('environment', 'name the working environment');
  if (!draft.source_hosts.length) issue('source_hosts', 'add at least one documentation host');
  for (const host of draft.source_hosts) if (!host.trim() || /[\/ ]/.test(host) || !host.includes('.')) issue('source_hosts', `${JSON.stringify(host)} is not a hostname`);
  if (draft.entry_points.map((e) => e.id).join(',') !== STAGES.map((s) => s.id).join(',')) issue('entry_points', 'the four stages must be foundations, mechanisms, production and synthesis, in that order');
  if (draft.entry_points.some((e) => !e.label.trim())) issue('entry_points', 'every stage needs a label');
  if (draft.topics.length < MIN_TOPICS) issue('topics', `a course needs at least ${MIN_TOPICS} topics`);
  if (draft.topics.length > MAX_TOPICS) issue('topics', `a course holds at most ${MAX_TOPICS} topics`);
  const slugs = new Set<string>(); const titles = new Set<string>();
  const index = new Map(draft.topics.map((t) => [t.slug, t]));
  for (const topic of draft.topics) {
    const at = `topics/${topic.slug}`;
    if (!topic.slug || !/^[a-z0-9-]+$/.test(topic.slug)) issue(at, 'a topic slug is lowercase letters, digits and hyphens');
    if (slugs.has(topic.slug)) issue(at, 'two topics share this slug'); slugs.add(topic.slug);
    if (!topic.title.trim()) issue(at, 'the topic needs a title');
    else if (titles.has(topic.title.trim().toLowerCase())) issue(at, 'two topics share this title'); else titles.add(topic.title.trim().toLowerCase());
    if (!topic.category.trim()) issue(at, 'give the topic a category');
    const b = topic.curriculum;
    if (!['foundations', 'mechanisms', 'production', 'synthesis', 'elective'].includes(b.phase)) issue(at, 'phase must be foundations, mechanisms, production, synthesis, or elective');
    if (words(b.learner_outcome) < 8) issue(at, 'learner outcome is too vague');
    if (b.mechanisms.filter((m) => m.trim()).length < 2) issue(at, 'at least two named mechanisms are required');
    if (words(b.production_scenario) < 8) issue(at, 'production scenario is too vague');
    if (!b.misconceptions.filter((m) => m.trim()).length) issue(at, 'at least one misconception is required');
    if (words(b.evidence) < 6) issue(at, 'observable evidence is too vague');
    if (words(b.artifact) < 6) issue(at, 'cumulative artifact is too vague');
    if (b.primary_sources.length < 2 || b.primary_sources.some((s) => !/^https?:\/\//.test(s))) issue(at, 'at least two absolute primary-source URLs are required');
    for (const p of topic.prereqs) {
      const earlier = index.get(p);
      if (!earlier) issue(at, `prerequisite ${p} is not a topic`);
      else if (tier(earlier.curriculum.phase) > tier(b.phase)) issue(at, `prerequisite ${p} sits in a later stage`);
      if (p === topic.slug) issue(at, 'a topic cannot require itself');
    }
    for (const source of b.primary_sources) if (/^https?:\/\//.test(source) && !onHost(draft.source_hosts, source)) issue(at, `source ${source} is not on one of the course's hosts`);
  }
  for (const stage of STAGES) if (!draft.topics.some((t) => t.curriculum.phase === stage.id && t.curriculum.core)) issue('topics', `the ${stage.id} stage needs at least one core topic`);
  const visited = new Set<string>();
  const visit = (slug: string, visiting: Set<string>): boolean => {
    if (visited.has(slug)) return true;
    if (visiting.has(slug)) return false;
    visiting.add(slug);
    for (const p of index.get(slug)?.prereqs ?? []) if (!visit(p, visiting)) return false;
    visiting.delete(slug); visited.add(slug); return true;
  };
  for (const topic of draft.topics) if (!visit(topic.slug, new Set())) issue(`topics/${topic.slug}`, 'prerequisites go round in a circle');
  return issues;
}

export function blankTopic(phase: DraftTopic['curriculum']['phase'] = 'foundations'): DraftTopic {
  return { slug: '', title: '', category: 'fundamentals', prereqs: [], curriculum: { phase, core: true, learner_outcome: '', mechanisms: ['', ''], production_scenario: '', misconceptions: [''], evidence: '', artifact: '', primary_sources: ['', ''], related_concepts: [] } };
}

function shortCode(title: string): string {
  const code = title.split(/\s+/).map((w) => w.replace(/[^a-z0-9]/gi, '')[0]).filter(Boolean).slice(0, 3).join('').toUpperCase();
  return code || 'MY';
}

export function blankDraft(id: string, brief: CourseBrief): CourseDraft {
  return { id, label: brief.title.trim(), native_label: '', short_code: shortCode(brief.title), title: brief.title.trim(), summary: '', context: '', outcome: brief.outcome.trim(), environment: '', source_hosts: [...brief.trusted_hosts], entry_points: STAGES.map((s) => ({ ...s })), topics: [{ ...blankTopic('foundations'), slug: 'first-topic', title: 'First topic' }] };
}

/** What the tutor would draft for the brief, in the preview. */
export function cannedDraft(id: string, brief: CourseBrief): CourseDraft {
  const hosts = brief.trusted_hosts.length ? [...brief.trusted_hosts] : ['doc.rust-lang.org', 'docs.rs'];
  const [h1, h2] = [hosts[0], hosts[1] ?? hosts[0]];
  const topic = (slug: string, title: string, phase: DraftTopic['curriculum']['phase'], core: boolean, prereqs: string[], category = 'fundamentals'): DraftTopic => ({
    slug, title, category, prereqs,
    curriculum: { phase, core, learner_outcome: `Explain ${title.toLowerCase()} with a worked example and a measured result of your own`, mechanisms: ['the mechanism underneath', 'what the tool does with it'], production_scenario: `A tool in daily use meets ${title.toLowerCase()} on a busy day and someone has to fix it before lunch`, misconceptions: ['it is not magic; there is a rule and it can be observed'], evidence: 'A trace showing the mechanism at work in the tool', artifact: 'A short note with the trace and the fix applied', primary_sources: [`https://${h1}/`, `https://${h2}/`], related_concepts: [] },
  });
  return {
    id, label: brief.title.trim(), native_label: 'your own course', short_code: shortCode(brief.title), title: brief.title.trim(),
    summary: `A self-study course so you can ${brief.outcome.trim().replace(/\.$/, '').toLowerCase()}.`,
    context: 'Teach each idea through something the learner runs and measures on their own machine, comparing each choice with what actually happens, and naming the mechanism before the tool.',
    outcome: `${brief.outcome.trim().replace(/\.$/, '')}, and explain each design choice with evidence from your own runs.`,
    environment: 'A terminal, a text editor and the toolchain the subject needs.',
    source_hosts: hosts,
    entry_points: [{ id: 'foundations', label: 'First steps' }, { id: 'mechanisms', label: 'How it works' }, { id: 'production', label: 'Under real constraints' }, { id: 'synthesis', label: 'The capstone' }],
    topics: [
      topic('setting-up', 'Setting up and running the first program', 'foundations', true, []),
      topic('values-and-types', 'Values, types and the first surprises', 'foundations', true, ['setting-up']),
      topic('control-flow', 'Control flow and early returns', 'foundations', true, ['values-and-types']),
      topic('errors', 'Errors as values, not events', 'mechanisms', true, ['control-flow'], 'mechanisms'),
      topic('modules', 'Modules, visibility and the build', 'mechanisms', true, ['setting-up'], 'mechanisms'),
      topic('io', 'Reading input and writing output', 'mechanisms', false, ['errors'], 'mechanisms'),
      topic('arguments', 'Parsing arguments and giving help', 'production', true, ['errors', 'modules'], 'tools'),
      topic('testing', 'Tests that run in a second', 'production', true, ['modules'], 'tools'),
      topic('performance', 'Measuring before optimising', 'production', false, ['io'], 'tools'),
      topic('release', 'Building a release and shipping it', 'synthesis', true, ['arguments', 'testing'], 'capstone'),
      topic('capstone', 'The capstone tool, end to end', 'synthesis', true, ['release'], 'capstone'),
    ],
  };
}

interface Stored extends CustomCourseView { text?: string }
const stored = new Map<string, Stored>();

/** The mock desk registers a program for a published class through this. */
let publishHook: ((course: CourseDefinition) => void) | null = null;
export function onPreviewPublish(hook: (course: CourseDefinition) => void) { publishHook = hook; }

function now() { return new Date().toISOString(); }
function view(id: string): CustomCourseView {
  const item = stored.get(id);
  if (!item) throw new Error('this class does not exist');
  return { ...item, issues: validateDraft(item.draft), draft: structuredClone(item.draft), brief: { ...item.brief }, review: [...item.review], sources: [...item.sources], bank: item.bank ? structuredClone(item.bank) : null };
}

function newId(title: string): string {
  const base = `custom-${slugify(title) || 'class'}`;
  let id = base; let n = 2;
  while (stored.has(id)) id = `${base}-${n++}`;
  return id;
}

export const previewCustom = {
  list: async (): Promise<CustomCourseSummary[]> => [...stored.values()].sort((a, b) => b.updated_at.localeCompare(a.updated_at)).map((c) => ({ id: c.id, label: c.draft.label, version: c.version, status: c.status, origin: c.origin, topics: c.draft.topics.length, updated_at: c.updated_at })),
  get: async (id: string) => view(id),
  create: async (brief: CourseBrief, origin: CustomCourseView['origin']) => {
    if (!brief.title.trim()) throw new Error('give the class a title');
    if (words(brief.outcome) < 5) throw new Error('say what you want to be able to do, in a sentence at least');
    const id = newId(brief.title);
    stored.set(id, { id, version: 0, status: 'draft', origin, brief: { ...brief }, draft: blankDraft(id, brief), issues: [], review: [], sources: [], bank: null, created_at: now(), updated_at: now(), published_at: null });
    return view(id);
  },
  saveBrief: async (id: string, brief: CourseBrief) => { const item = stored.get(id); if (!item) throw new Error('this class does not exist'); item.brief = { ...brief }; item.updated_at = now(); return view(id); },
  saveDraft: async (id: string, draft: CourseDraft) => {
    const item = stored.get(id); if (!item) throw new Error('this class does not exist');
    item.draft = structuredClone({ ...draft, id, topics: draft.topics.map((t) => ({ ...t, slug: slugify(t.slug || t.title) })) });
    item.updated_at = now(); return view(id);
  },
  draft: async (id: string) => { const item = stored.get(id); if (!item) throw new Error('this class does not exist'); await new Promise((r) => setTimeout(r, 1800)); item.draft = cannedDraft(id, item.brief); item.updated_at = now(); return view(id); },
  review: async (id: string) => {
    const item = stored.get(id); if (!item) throw new Error('this class does not exist');
    await new Promise((r) => setTimeout(r, 1200));
    const first = item.draft.topics[0]?.slug ?? '';
    const findings: ReviewFinding[] = [
      { severity: 'medium', topic: first, message: 'The first topic assumes a toolchain is installed; a learner starting from nothing has no step for that.', fix: 'Add an installation-and-first-run step before it, or fold one into its lesson outcome.' },
      { severity: 'low', topic: '', message: 'The capstone stage has no elective; a learner who finishes early has nowhere to go.', fix: 'Add one elective topic that extends the capstone.' },
    ];
    item.review = findings; item.updated_at = now(); return view(id);
  },
  verify: async (id: string) => {
    const item = stored.get(id); if (!item) throw new Error('this class does not exist');
    await new Promise((r) => setTimeout(r, 900));
    const checks: SourceCheck[] = [];
    for (const topic of item.draft.topics) for (const url of topic.curriculum.primary_sources) checks.push({ topic: topic.slug, url, state: !onHost(item.draft.source_hosts, url) ? 'off-host' : /example\.com|invalid/.test(url) ? 'unreachable' : 'reachable' });
    item.sources = checks; item.updated_at = now(); return view(id);
  },
  writeBank: async (id: string) => {
    const item = stored.get(id); if (!item) throw new Error('this class does not exist');
    const issues = validateDraft(item.draft);
    if (issues.length) throw new Error(`fix the draft before writing its questions: ${issues[0].message}`);
    await new Promise((r) => setTimeout(r, 1500));
    const stages = ['foundations', 'mechanisms', 'production', 'synthesis'];
    const questions: QuestionBank['questions'] = [];
    for (const stage of stages) {
      const core = item.draft.topics.filter((t) => t.curriculum.phase === stage && t.curriculum.core);
      for (let i = 0; i < 3; i++) {
        const topic = core[i % core.length];
        const n = questions.length + 1;
        questions.push({ id: `${id}-entry-${n}`, criterion: `${id}-criterion-${n}`, competency: `${id}-${topic.slug}`, entry_point: stage, label: `${topic.title.split(' ').slice(0, 3).join(' ')}`, prompt: `In ${topic.title.toLowerCase()}, which statement holds?`, choices: [{ id: '1', text: 'The first plausible mistake' }, { id: '2', text: 'The mechanism as the source describes it' }, { id: '3', text: 'A common misconception' }, { id: '4', text: 'An unrelated fact' }], answer: '2', explanation: 'The source names the mechanism; the others are the misconceptions the lesson warns about.', followup_for: null, source: topic.curriculum.primary_sources[0] ?? '' });
      }
    }
    item.bank = { course_id: id, version: `written-${Date.now()}`, estimated_minutes: 14, scope_note: 'Twelve short questions, three for each stage of this course, written by the tutor from the course\'s own sources.', questions };
    item.updated_at = now(); return view(id);
  },
  voidQuestion: async (id: string, questionId: string, reason: string) => {
    const item = stored.get(id); if (!item?.bank) throw new Error('this class has no question bank');
    const q = item.bank.questions.find((q) => q.id === questionId); if (!q) throw new Error('that question is not in the bank');
    q.voided = true; q.void_reason = reason.trim().slice(0, 400); item.updated_at = now(); return view(id);
  },
  publish: async (id: string) => {
    const item = stored.get(id); if (!item) throw new Error('this class does not exist');
    const issues = validateDraft(item.draft);
    if (issues.length) throw new Error(`the draft is not ready to publish: ${issues[0].message} (${issues[0].at} and ${issues.length - 1} more)`);
    item.version += 1; item.status = 'published'; item.published_at = now(); item.updated_at = now();
    const definition = definitionOf(item);
    registerCourses([definition]);
    publishHook?.(definition);
    return view(id);
  },
  remove: async (id: string) => { const item = stored.get(id); if (!item) throw new Error('this class does not exist'); if (item.status === 'published') throw new Error('a published class keeps its lessons; retire it from its settings instead'); stored.delete(id); },
  export: async (id: string): Promise<ClassExport> => { const item = view(id); const name = `${item.draft.label.replace(/[^a-z0-9 -]/gi, '-').trim() || id}.principia-class.json`; return { path: `~/Documents/Principia Desk/classes/${name}`, file_name: name }; },
  import: async (text: string) => {
    let file: { format?: string; brief?: CourseBrief; draft?: CourseDraft };
    try { file = JSON.parse(text); } catch (error) { throw new Error(`not a class file: ${error}`); }
    if (file.format !== 'principia-class/1' || !file.draft) throw new Error(`this is not a class file the desk understands (${file.format ?? 'no format'})`);
    const brief: CourseBrief = { title: '', outcome: '', background: '', trusted_hosts: [], agent: '', model: '', custom_agent_bin: '', ...(file.brief ?? {}) };
    if (!brief.title.trim()) brief.title = file.draft.label;
    if (words(brief.outcome) < 5) brief.outcome = file.draft.outcome;
    const created = await previewCustom.create(brief, 'import');
    return previewCustom.saveDraft(created.id, { ...file.draft, id: created.id });
  },
  /** The published courses, for the preview's catalogue. */
  published: (): CourseDefinition[] => [...stored.values()].filter((c) => c.status === 'published').map(definitionOf),
};

function definitionOf(item: Stored): CourseDefinition {
  const d = item.draft;
  return { id: d.id as CourseDefinition['id'], course_id: d.id, kind: 'engineering', label: d.label, native_label: d.native_label || 'your own course', short_code: d.short_code, title: d.title, summary: d.summary, version: `v${item.version}`, context: d.context, outcome: d.outcome, environment: d.environment, prompt_profile: `custom.${d.id}`, prerequisite_courses: [], source_hosts: [...d.source_hosts], reference_lessons: [], capabilities: ['explanation', 'knowledge_checks', 'code', 'diagrams', 'tutor'], entry_points: d.entry_points.map((e) => ({ ...e })) };
}
