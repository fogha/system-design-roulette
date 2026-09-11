import { previewConfiguration, savePreviewConfiguration, previewRunners, previewModels, previewLocal, rememberPreviewModel, desktopRequired } from './features/runners/preview';
import type { FocusPolicy, CurriculumConceptView, RouteSummary } from './ipc';
import type { RevisePath } from './contracts/classes';
import type { AgentPolicy, RunnerId } from './contracts/agents';
import { previewEnrollmentOptions, previewEnrollmentDraft, savePreviewEnrollmentDraft } from './enrollment-preview';
import { getPreviewPlacement, startPreviewPlacement, savePreviewPlacement, submitPreviewPlacement, continuePreviewPlacement, finishPreviewPlacement, recommendPreviewPath } from './placement-preview';
import type { SaveEnrollmentDraft } from './contracts/enrollment';
import type { AcceptedPath, AcceptPath } from './contracts/classes';
import { acceptPreviewClassPath, previewClassPath, previewPathSummary, revisePreviewClassPath } from './class-preview';
import { applyPreviewChallenge, getPreviewChallenge, savePreviewChallengeResponse, startPreviewChallenge, submitPreviewChallenge } from './challenge-preview';
import { scheduleConflicts, conflictMessage, type ScheduleCandidate, type ConflictScope } from './features/classes/schedule-conflicts';
import { COURSES, courseDefinition } from './catalog';
import seedConcepts from '../../src-tauri/seed/concepts.json';
/**
 * Demo-mode API: used automatically when the frontend runs outside Tauri
 * (plain `vite dev` in a browser). Lets you develop and screenshot every
 * screen without the Rust backend. Same shapes as ipc.ts.
 */
import type {
  AppStateView,
  ProgressQuery,
  ProgressEntry,
  ProgressLesson,
  ChatMessage,
  DashboardView,
  ExerciseView,
  CefrLevel,
  ClassroomPlanView,
  ClassroomProgramView,
  ClassroomSessionStart,
  ClassroomSlotView,
  ClassroomSubjectId,
  CurriculumBrief,
  CurriculumMapView,
  EngineeringLessonView,
  EngineeringSessionResult,
  FocusArea,
  LanguageId,
  LanguageLessonView,
  LanguageProgramView,
  LanguageSessionResult,
  PlannedSlot,
} from './ipc';

// References and course metadata share the native authoring inputs. Browser
// demos should never label an event-loop lesson as Bash or System Design.
interface ReferenceLesson {
  slug: string;
  title: string;
  markdown: string;
  resources: EngineeringLessonView['resources'];
  exercise: EngineeringLessonView['exercise'];
  questions: { prompt: string; kind: string; choices: string[] | null; correct_answer: string; explanation: string; section: string; learning_objective: string }[];
}
const referenceFiles = import.meta.glob<ReferenceLesson>('../../src-tauri/seed/fallback_courses/*.json', { eager: true, import: 'default' });
function referenceFor(focus: FocusArea): ReferenceLesson {
  const slug = courseDefinition(focus)!.reference_lessons[0];
  const lesson = Object.values(referenceFiles).find((lesson) => lesson.slug === slug);
  if (!lesson) throw new Error(`Missing reference lesson for ${focus}`);
  return lesson;
}

type Handler = (payload: unknown) => void;
const listeners = new Map<string, Handler[]>();

export function mockEmit(name: string, payload: unknown) {
  for (const h of listeners.get(name) ?? []) h(payload);
}

export function mockListen(name: string, handler: Handler): () => void {
  const arr = listeners.get(name) ?? [];
  arr.push(handler);
  listeners.set(name, arr);
  return () => listeners.set(name, (listeners.get(name) ?? []).filter((h) => h !== handler));
}

const COURSE_MD = `## Why this matters

Every async API in JavaScript — \`fetch\`, \`setTimeout\`, DOM events, promises — funnels through the same scheduling machinery. Misunderstanding which queue runs when is how you ship subtle ordering bugs, starvation under load, and "it works in the test but hangs in prod" failures. The event loop is the mental model that ties runtime, browser, and Node together.

## The simple version

Think of the event loop as a chef who always finishes every add-on ticket for the dish in front of them (microtasks) before ever glancing at the next new order that walked in (macrotasks).

\`\`\`mermaid
flowchart LR
  A[Run current macrotask] --> B{Microtask spike empty?}
  B -- no --> C[Run one microtask]
  C --> B
  B -- yes --> D[Optional render]
  D --> E[Take next macrotask]
\`\`\`

Where this breaks down: a real chef eventually moves on regardless. The event loop does not — a microtask that keeps enqueueing more microtasks can starve macrotasks (and rendering) indefinitely.

## Core mechanics

### Macrotasks vs microtasks

The event loop drains **one macrotask** (timer callback, I/O completion, user event), then **all pending microtasks** (promise reactions, \`queueMicrotask\`), then may render. Microtasks always run before the next macrotask — so a chain of \`Promise.then\` can starve timers if you recurse without yielding.

\`\`\`js
console.log('sync');
setTimeout(() => console.log('macro'), 0);
Promise.resolve().then(() => console.log('micro'));
// sync → micro → macro
\`\`\`

### The call stack and host APIs

JS runs on a single call stack per agent. Host environments enqueue work: the timer thread schedules macrotasks; the network layer resolves fetch promises as microtasks. Your code never "blocks the loop" with promises — it blocks with **synchronous** CPU work on the stack.

### \`await\` and continuation scheduling

\`await\` suspends an async function and resumes via a microtask when the operand settles. That means async/await ordering matches promise \`.then\` ordering, and errors propagate through the same microtask turn unless you \`await\` inside try/catch.

## Trade-offs and failure modes

- Microtask storms: unbounded \`queueMicrotask\` recursion prevents paint and timer delivery.
- \`setTimeout(fn, 0)\` is not "run next" — it is "run after current macrotask **and** all microtasks."
- In browsers, \`requestAnimationFrame\` runs before paint; confusing it with microtasks breaks frame-aligned work.

## Interview framing

State the loop as: run script → macrotask → microtask checkpoint (repeat). Give the sync/micro/macro log ordering example, then explain starvation. Close with where \`await\` schedules — microtask, same turn as the resolving promise.`;

const MOCK_COURSE_ID = 42;

const MOCK_EXERCISE: Omit<ExerciseView, 'draft'> = {
  course_id: MOCK_COURSE_ID,
  classroom_session_id: null,
  study_session_id: null,
  title: 'Trace and tame a microtask storm',
  instructions:
    'Write a tiny script that logs a numbered tag for each callback so you can see the exact order the event loop runs things in.\n\n1. Log a `sync-start` tag.\n2. Schedule a `setTimeout(..., 0)` that logs a `macrotask` tag.\n3. Chain two `.then()` calls off a resolved promise, each logging a `microtask` tag.\n4. Log a `sync-end` tag.\n5. Run it and annotate which line ran in which "wave" (sync, microtask checkpoint, macrotask).',
  starter_code:
    "let n = 0;\nconst tag = (label) => console.log(`${n++} ${label}`);\n\ntag('sync-start');\nsetTimeout(() => tag('macrotask'), 0);\nPromise.resolve().then(() => tag('microtask-1')).then(() => tag('microtask-2 (nested)'));\ntag('sync-end');\n",
  deliverable:
    'A numbered console log with your own annotation (macrotask/microtask) next to each line, plus one sentence on where the nested .then() landed.',
  hints: [
    'Run the sync lines first on paper — what fires before any callback gets a chance to run at all?',
    'Both microtask callbacks are on the SAME queue as any other promise reaction — they drain completely before the timer fires.',
    'The second .then() is only enqueued once the first one runs, so it lands in a later microtask checkpoint, not the same one.',
  ],
  completed: false,
  reflection: '',
};

let mockExerciseDraft: string | null = null;
let mockExerciseCompleted = false;
let mockExerciseReflection = '';

// Session-only chat: in-memory per course id, cleared the same way the real
// backend clears it (completion, skip, extension, new session).
const mockChatThreads = new Map<number, ChatMessage[]>();
const mockClassroomChatThreads = new Map<number, ChatMessage[]>();
const mockClassroomExerciseDrafts = new Map<number, string>();
const mockClassroomExerciseCompletions = new Map<
  number,
  { completed: boolean; reflection: string }
>();
function clearMockChatThreads() {
  mockChatThreads.clear();
  mockClassroomChatThreads.clear();
}

const params =
  typeof location !== 'undefined' ? new URLSearchParams(location.search) : new URLSearchParams();
const jump = params.get('step');
const skippedPreview = params.has('skipped');

let setupCompleted = false;
let mockPaused = params.has('paused');
let mockKioskLevel = (params.get('kiosk') ?? 'hard') as AppStateView['kiosk_level'];
let mockModel = (params.get('model') ?? 'opus') as AppStateView['model'];
let mockAgent = (params.get('agent') ?? 'claude') as AppStateView['agent'];
let mockCustomBin = '';
let mockSelectedFocus: FocusArea = 'javascript';
let mockDeepseekKeyConfigured = params.has('deepseekKey');
let mockActiveLanguage: LanguageLessonView | null = null;
let mockLanguageSessionId = 900;
const mockLanguageSettings: Record<
  LanguageId,
  {
    enabled: boolean;
    startLevel: CefrLevel;
    currentLevel: CefrLevel;
    targetLevel: CefrLevel;
    weeklyMinutes: number;
    sessionMinutes: number;
  }
> = {
  german: {
    enabled: params.get('program') === 'german',
    startLevel: 'A1',
    currentLevel: 'A1',
    targetLevel: 'A2',
    weeklyMinutes: 210,
    sessionMinutes: 30,
  },
  italian: {
    enabled: params.get('program') === 'italian',
    startLevel: 'A1',
    currentLevel: 'A1',
    targetLevel: 'A2',
    weeklyMinutes: 210,
    sessionMinutes: 30,
  },
};
const CLASSROOM_CATALOG = COURSES.map((course) => ({
  id: course.id, kind: course.kind, label: course.label,
  native: course.native_label, short: course.short_code,
}));

const requestedClass = (params.get('class') ?? params.get('program')) as
  | ClassroomSubjectId
  | null;
const mockClassroomSettings = Object.fromEntries(
  CLASSROOM_CATALOG.map((item) => [
    item.id,
    {
      enabled:
        requestedClass === item.id ||
        (item.kind === 'language' && mockLanguageSettings[item.id as LanguageId].enabled),
      agent: mockAgent,
      model: mockModel,
      customBin: '',
      sessionMinutes: 30,
      learningGoal: '',
      targetWeeklyMinutes: 0,
      focusPolicy: 'advisory' as FocusPolicy,
    },
  ]),
) as Record<
  ClassroomSubjectId,
  {
    enabled: boolean;
    agent: AppStateView['agent'];
    model: AppStateView['model'];
    customBin: string;
    sessionMinutes: number;
    learningGoal: string;
    targetWeeklyMinutes: number;
    focusPolicy: FocusPolicy;
  }
>;
let mockClassroomSlots: ClassroomSlotView[] = requestedClass
  ? [
      {
        id: 801,
        subject_id: requestedClass,
        label:
          CLASSROOM_CATALOG.find((item) => item.id === requestedClass)?.label ?? requestedClass,
        short_code:
          CLASSROOM_CATALOG.find((item) => item.id === requestedClass)?.short ?? 'CL',
        kind:
          CLASSROOM_CATALOG.find((item) => item.id === requestedClass)?.kind ?? 'engineering',
        hour: 7,
        minute: 30,
        weekdays: [1, 2, 3, 4, 5, 6], durations: {},
        enabled: true,
        owed: params.has('classDue') || params.has('languageDue'),
        next_fire_at: new Date(Date.now() + 4 * 60 * 60 * 1000).toISOString().slice(0, 19),
        in_progress: false,
        source: 'manual',
      occurrence_id: null,
      disposition: null,
      },
    ]
  : [];
let mockActiveEngineering: EngineeringLessonView | null = null;
let mockEngineeringSessionId = 1000;

function addDays(days: number) {
  const date = new Date();
  date.setDate(date.getDate() + days);
  return date.toISOString().slice(0, 10);
}

function mockLanguageProgram(language: LanguageId): LanguageProgramView {
  const settings = mockLanguageSettings[language];
  const isGerman = language === 'german';
  return {
    language,
    label: isGerman ? 'German' : 'Italian',
    native_label: isGerman ? 'Deutsch' : 'Italiano',
    enabled: settings.enabled,
    start_level: settings.startLevel,
    current_level: settings.currentLevel,
    target_level: settings.targetLevel,
    start_date: new Date().toISOString().slice(0, 10),
    target_date: addDays(settings.targetLevel === 'A1' ? 30 : 90),
    weekly_minutes: settings.weeklyMinutes,
    session_minutes: settings.sessionMinutes,
    completed_steps: settings.enabled ? 7 : 0,
    required_steps: 30,
    progress: settings.enabled ? 7 / 30 : 0,
    total_completed_steps: settings.enabled ? 7 : 0,
    total_required_steps: settings.targetLevel === 'A1' ? 30 : 90,
    recommended_weekly_minutes: isGerman ? 700 : 420,
    pace_status: 'commitment_gap',
    pace_message:
      `The ${settings.targetLevel} timeline needs more total weekly practice than the current commitment. App sessions are only one part.`,
    milestones: [
      { level: 'A1', target_date: addDays(30), reached: false },
      { level: 'A2', target_date: addDays(90), reached: false },
      { level: 'B1', target_date: addDays(270), reached: false },
      { level: 'B2', target_date: addDays(540), reached: false },
    ],
    skills: [
      { id: 'listening', label: 'Listening', score: 0.62, encounters: 4 },
      { id: 'reading', label: 'Reading', score: 0.74, encounters: 5 },
      { id: 'spoken_interaction', label: 'Spoken interaction', score: 0.48, encounters: 2 },
      { id: 'spoken_production', label: 'Spoken production', score: 0.51, encounters: 2 },
      { id: 'writing', label: 'Writing', score: 0.58, encounters: 3 },
      { id: 'grammar', label: 'Grammar', score: 0.68, encounters: 6 },
      {
        id: 'vocabulary_pragmatics',
        label: 'Vocabulary pragmatics',
        score: 0.71,
        encounters: 7,
      },
    ],
    official_sources: isGerman
      ? [
          {
            title: 'Goethe-Institut — German examinations',
            url: 'https://www.goethe.de/en/spr/prf.html',
          },
        ]
      : [
          {
            title: 'Centro CILS',
            url: 'https://cils.unistrasi.it/',
          },
        ],
  };
}

function mockLanguageLesson(language: LanguageId): LanguageLessonView {
  const german = language === 'german';
  const target = german ? 'Deutsch' : 'Italiano';
  return {
    session_id: String(mockLanguageSessionId),
    runtime: 'legacy',
    lifecycle: 'in_progress',
    revision: 0,
    checkpoint: null,
    check: null,
    outcome: null,
    language,
    label: german ? 'German' : 'Italian',
    native_label: target,
    level: 'A1',
    unit_slug: german ? 'de-a1-greetings-introductions' : 'it-a1-greetings',
    phase: 1,
    phase_requirement: null,
    phase_label: 'notice and understand',
    title: german ? 'Greetings and Introductions' : 'Saluti e presentazioni',
    scenario: german
      ? 'You arrive at a language course in Berlin and meet the teacher.'
      : 'You arrive at an Italian course in Rome and meet the teacher.',
    can_do: german
      ? 'I can greet people, introduce myself, and ask basic personal questions.'
      : 'I can greet people, introduce myself, and ask how someone is.',
    markdown: german
      ? `## Mission

You arrive at a language course in Berlin. Your goal is not to recite a vocabulary list: it is to complete a first meeting politely.

## Model dialogue

- **Lehrerin:** Guten Morgen! Wie heißen Sie?
  _Good morning! What is your name?_
- **Lernende:** Ich heiße Maria Santos.
  _My name is Maria Santos._
- **Lehrerin:** Woher kommen Sie?
  _Where are you from?_

## Grammar in service of the task

Use **ich heiße** for your name and **ich komme aus** for your origin. German keeps the verb in second position in these statements.

## Pragmatics

Use **Sie** with an unfamiliar adult until invited to use **du**. The distinction is social, not merely grammatical.`
      : `## Mission

You arrive at a language course in Rome. Complete a polite first meeting instead of translating isolated words.

## Model dialogue

- **Insegnante:** Buongiorno! Come si chiama?
  _Good morning! What is your name?_
- **Studente:** Mi chiamo Marco. Piacere.
  _My name is Marco. Nice to meet you._
- **Insegnante:** Di dove è?
  _Where are you from?_

## Grammar in service of the task

Use **mi chiamo** to introduce your name. Subject pronouns are often omitted because the verb ending carries the person.

## Pragmatics

Use **Lei** and the third-person verb with unfamiliar adults; **tu** belongs with peers and friends.`,
    phrases: german
      ? [
          { target: 'Wie heißen Sie?', translation: 'What is your name?', note: 'Formal' },
          { target: 'Ich komme aus …', translation: 'I am from …', note: 'Origin' },
        ]
      : [
          { target: 'Come si chiama?', translation: 'What is your name?', note: 'Formal' },
          { target: 'Mi chiamo …', translation: 'My name is …', note: 'Introduction' },
        ],
    dialogue: german
      ? [
          { speaker: 'Lehrerin', target: 'Guten Morgen! Wie heißen Sie?', translation: 'Good morning! What is your name?' },
          { speaker: 'Lernende', target: 'Ich heiße Maria Santos.', translation: 'My name is Maria Santos.' },
        ]
      : [
          { speaker: 'Insegnante', target: 'Buongiorno! Come si chiama?', translation: 'Good morning! What is your name?' },
          { speaker: 'Studente', target: 'Mi chiamo Marco. Piacere.', translation: 'My name is Marco. Nice to meet you.' },
        ],
    questions: [
      {
        id: 1,
        prompt: german ? 'What does “Guten Morgen” mean?' : 'What does “Buongiorno” mean?',
        choices: ['Good morning', 'Good night', 'Please', 'Thank you'],
        strand: 'vocabulary_pragmatics',
      },
      {
        id: 2,
        prompt: german ? 'Complete: Ich ___ Maria.' : 'Complete: Mi ___ Maria.',
        choices: german ? ['heiße', 'komme', 'bin aus', 'habe'] : ['chiamo', 'sono di', 'ho', 'stai'],
        strand: 'grammar',
      },
      {
        id: 3,
        prompt: german ? 'Which form is polite with a stranger?' : 'Which form is polite with a stranger?',
        choices: german ? ['Wie heißen Sie?', 'Wie heißt du?', 'Tschüss du!', 'Wo wohnst du?'] : ['Come si chiama?', 'Come ti chiami?', 'Ciao tu!', 'Dove abiti tu?'],
        strand: 'spoken_interaction',
      },
      {
        id: 4,
        prompt: german ? 'How do you say “I am from …”?' : 'How do you say “My name is …”?',
        choices: german ? ['Ich komme aus …', 'Ich heiße aus …', 'Ich habe aus …', 'Ich bin Name …'] : ['Mi chiamo …', 'Sono chiamo …', 'Ho nome …', 'Io chiamare …'],
        strand: 'spoken_production',
      },
      {
        id: 5,
        prompt: german ? 'What social distinction do Sie and du express?' : 'What social distinction do Lei and tu express?',
        choices: ['Formality and relationship', 'Past and present', 'Singular and plural only', 'Written and spoken language'],
        strand: 'vocabulary_pragmatics',
      },
    ],
    speaking_prompt: german
      ? 'Introduce yourself with your name, origin, and a polite closing.'
      : 'Introduce yourself with your name, origin, and a polite closing.',
    writing_prompt: german
      ? 'Write six short sentences introducing yourself to a course teacher.'
      : 'Write six short sentences introducing yourself to a course teacher.',
    listen_text: german
      ? 'Guten Morgen! Wie heißen Sie? Ich heiße Maria Santos. Woher kommen Sie?'
      : 'Buongiorno! Come si chiama? Mi chiamo Marco. Piacere. Di dove è?',
    speech_locale: german ? 'de-DE' : 'it-IT',
    estimated_minutes: 30,
    status: 'in_progress',
  };
}

const restoredPaths = new Map<ClassroomSubjectId, string>();
function applyPreviewPath(path: AcceptedPath | null) {
  if (!path) return;
  const id = path.recommendation.course.course_id;
  if (restoredPaths.get(id) === path.reference.path_revision_id) return;
  const { tutor, pace, goal } = path.configuration;
  const scheduled = mockClassroomSlots.some(slot => slot.subject_id === id && slot.enabled);
  Object.assign(mockClassroomSettings[id], { enabled: scheduled, agent: tutor.provider, model: tutor.model, customBin: tutor.custom_agent_bin ?? '', sessionMinutes: pace.session_minutes, learningGoal: goal.note, targetWeeklyMinutes: pace.weekly_minutes ?? mockClassroomSettings[id].targetWeeklyMinutes });
  if ((id === 'german' || id === 'italian') && goal.kind === 'language_level') {
    const settings = mockLanguageSettings[id];
    Object.assign(settings, { enabled: scheduled, currentLevel: path.recommendation.entry_point, targetLevel: goal.target_level, sessionMinutes: pace.session_minutes, weeklyMinutes: pace.weekly_minutes ?? settings.weeklyMinutes });
  }
  restoredPaths.set(id, path.reference.path_revision_id);
}
/** Preview counterpart of the native route summary: same denominators, no credit. */
function previewRoute(subjectId: ClassroomSubjectId, acceptedPath: AcceptedPath | null): RouteSummary | null {
  const plan = acceptedPath?.recommendation;
  if (!acceptedPath || !plan) return null;
  const listed = (list: { id: string }[] | undefined, slug: string) => !!list?.some((topic) => topic.id === slug);
  const concepts = seedConcepts.filter((concept) => concept.focus === subjectId);
  const setAside = (slug: string) => listed(plan.earlier_topics, slug) || listed(plan.bypassed, slug) || listed(plan.checked, slug);
  const core = concepts.filter((concept) => concept.curriculum.core);
  const order = ['foundations', 'mechanisms', 'production', 'synthesis', 'elective'];
  const bridge = plan.bridges?.[0];
  const candidate = bridge ? concepts.find((concept) => concept.slug === bridge.id) : [...core].sort((a, b) => order.indexOf(a.curriculum.phase) - order.indexOf(b.curriculum.phase) || a.tier - b.tier).find((concept) => !setAside(concept.slug));
  return {
    revision: acceptedPath.revision,
    entry_label: plan.entry_label,
    required_total: core.filter((concept) => !setAside(concept.slug)).length,
    required_done: 0,
    coverage_total: core.length,
    coverage_done: 0,
    demonstrated: (plan.checked?.length ?? 0) + plan.criteria.filter((row) => row.verdict === 'passed').length,
    needs_review: plan.refreshers.length + (plan.bridges?.length ?? 0),
    next: candidate ? { slug: candidate.slug, title: candidate.title, reason: bridge ? `Bridge lesson before ${bridge.reason.replace(/^Bridge before /, '').replace(/\.$/, '')}.` : 'Next required topic on your accepted route.' } : null,
    stale: false,
  };
}

function mockClassroomProgram(subjectId: ClassroomSubjectId): ClassroomProgramView {
  const acceptedPath = previewClassPath(subjectId);
  applyPreviewPath(acceptedPath);
  const catalog = CLASSROOM_CATALOG.find((item) => item.id === subjectId)!;
  const settings = mockClassroomSettings[subjectId];
  const languageProgress =
    catalog.kind === 'language' ? mockLanguageProgram(subjectId as LanguageId) : null;
  return {
    subject_id: subjectId,
    accepted_path: previewPathSummary(acceptedPath),
    kind: catalog.kind,
    label: catalog.label,
    native_label: catalog.native,
    short_code: catalog.short,
    enabled: settings.enabled && mockClassroomSlots.some(slot => slot.subject_id === subjectId && slot.enabled),
    agent: settings.agent,
    model: settings.model,
    custom_agent_bin: settings.customBin,
    prompt_profile: `classroom.${subjectId}`,
    prompt_version: 'v1',
    session_minutes: settings.sessionMinutes,
    learning_goal: settings.learningGoal,
    focus_policy: settings.focusPolicy,
    route: previewRoute(subjectId, acceptedPath),
    review_due: 0,
    target_weekly_minutes: settings.targetWeeklyMinutes,
    progress: languageProgress?.progress ?? 0,
    progress_label:
      languageProgress
        ? `${languageProgress.completed_steps} / ${languageProgress.required_steps} evidence steps in ${languageProgress.current_level}`
        : `0 / ${seedConcepts.filter((concept) => concept.focus === subjectId && concept.curriculum.core).length} core concepts practiced`,
    completed: false,
    language_progress: languageProgress,
  };
}

function mockEngineeringLesson(subjectId: FocusArea): EngineeringLessonView {
  const catalog = CLASSROOM_CATALOG.find((item) => item.id === subjectId)!;
  const reference = referenceFor(subjectId);
  const concept = seedConcepts.find((concept) => concept.slug === reference.slug)!;
  return {
    session_id: String(mockEngineeringSessionId),
    runtime: 'legacy',
    lifecycle: 'in_progress',
    revision: 0,
    checkpoint: null,
    check: null,
    outcome: null,
    subject_id: subjectId,
    label: catalog.label,
    short_code: catalog.short,
    title: reference.title,
    concept_slug: reference.slug,
    concept_title: concept.title,
    category: concept.category,
    curriculum: concept.curriculum as CurriculumBrief,
    prerequisites: concept.prereqs,
    session_index: 1,
    why_now: 'Browser preview: a bundled reference lesson from this course. Native selection follows curriculum eligibility.',
    markdown: reference.markdown,
    resources: reference.resources,
    questions: reference.questions.filter((question) => question.kind === 'mcq').map((question, index) => ({
      id: index + 1, prompt: question.prompt, choices: question.choices!,
      section: question.section, learning_objective: question.learning_objective,
    })),
    exercise: reference.exercise,
    kind: 'lesson' as const,
    fresh_sample: true,
    agent_used: mockClassroomSettings[subjectId].agent,
    prompt_profile: `classroom.${subjectId}`,
    prompt_version: `classroom.${subjectId}.v1`,
    estimated_minutes: mockClassroomSettings[subjectId].sessionMinutes,
    status: 'in_progress',
  };
}

function appState(): AppStateView {
  const inSetup = typeof location !== 'undefined' && location.search.includes('setup') && !setupCompleted;
  return {
    onboarded: !inSetup,
    selected_focus: mockSelectedFocus,
    // Recurring obligations now belong to classes; no separate daily trigger.
    schedule_hour: 19,
    schedule_minute: 0,
    debug_day: true,
    enforcement_disarmed: params.has('disarmed'),
    schedule_paused: mockPaused,
    kiosk_level: mockKioskLevel,
    model: mockModel,
    agent: mockAgent,
    custom_agent_bin: mockCustomBin,
    deepseek_key_configured: mockDeepseekKeyConfigured,
    alarm: null,
    classroom_programs: CLASSROOM_CATALOG.map((item) => mockClassroomProgram(item.id)),
    classroom_slots: mockClassroomSlots,
    classroom_due_count: mockClassroomSlots.filter((slot) => slot.owed).length,
    appointments: [],
    focus: null,
    active_classroom_sessions: [
      ...(mockActiveLanguage
        ? [
            {
              session_id: String(mockActiveLanguage.session_id),
              subject_id: mockActiveLanguage.language,
              kind: 'language' as const,
              label: mockActiveLanguage.label,
              title: mockActiveLanguage.title,
              runtime: 'legacy' as const,
              lifecycle: 'in_progress',
            },
          ]
        : []),
      ...(mockActiveEngineering
        ? [
            {
              session_id: mockActiveEngineering.session_id,
              subject_id: mockActiveEngineering.subject_id,
              kind: 'engineering' as const,
              label: mockActiveEngineering.label,
              title: mockActiveEngineering.title,
              runtime: 'legacy' as const,
              lifecycle: 'in_progress',
            },
          ]
        : []),
    ],
  };
}

let previewFreeOnly = true;

function mockPrograms(): ClassroomProgramView[] {
  return CLASSROOM_CATALOG.map((item) => mockClassroomProgram(item.id as ClassroomSubjectId));
}
function rejectMockConflicts(candidates: ScheduleCandidate[], scope: ConflictScope = {}) {
  const conflicts = scheduleConflicts(candidates, mockClassroomSlots, mockPrograms(), scope);
  if (conflicts.length) throw new Error(conflictMessage(conflicts));
}
function pauseMockClassWithoutSchedule(subjectId: ClassroomSubjectId) {
  if (mockClassroomSlots.some(slot => slot.subject_id === subjectId && slot.enabled)) return;
  mockClassroomSettings[subjectId].enabled = false;
  if (subjectId === 'german' || subjectId === 'italian') mockLanguageSettings[subjectId].enabled = false;
}

export const mockApi = {
  getEnrollmentOptions: async (courseId: ClassroomSubjectId) => {
    const options = previewEnrollmentOptions(courseId);
    options.default_configuration.tutor = { provider: mockAgent, model: mockModel, custom_agent_bin: mockCustomBin || null };
    const path = previewClassPath(courseId);
    if (path) options.default_configuration = structuredClone(path.configuration);
    return options;
  },
  getClassPath: async (courseId: ClassroomSubjectId) => previewClassPath(courseId),
  getUnitChallenge: getPreviewChallenge,
  startUnitChallenge: startPreviewChallenge,
  saveUnitChallengeResponse: savePreviewChallengeResponse,
  submitUnitChallengeRound: submitPreviewChallenge,
  applyUnitChallenge: applyPreviewChallenge,
  reviseClassPath: async (input: RevisePath) => revisePreviewClassPath(input, (slug) => seedConcepts.find((concept) => concept.slug === slug && concept.focus === input.course_id)?.title),
  acceptClassPath: async (input: AcceptPath) => {
    const path = await acceptPreviewClassPath(input);
    applyPreviewPath(previewClassPath(path.recommendation.course.course_id));
    mockEmit('classroom:state', previewPathSummary(path));
    return path;
  },
  getPlacementCheck: getPreviewPlacement,
  startPlacementCheck: startPreviewPlacement,
  savePlacementResponse: savePreviewPlacement,
  submitPlacementRound: submitPreviewPlacement,
  continuePlacementCheck: continuePreviewPlacement,
  finishPlacementCheck: finishPreviewPlacement,
  getPathRecommendation: recommendPreviewPath,
  getEnrollmentDraft: async (courseId: ClassroomSubjectId) => previewEnrollmentDraft(courseId),
  saveEnrollmentDraft: async (input: SaveEnrollmentDraft) => savePreviewEnrollmentDraft(input),
  getCatalog: async () => [...COURSES],
  getAppState: async () => appState(),
  checkAgent: async () => false,
  getRunnerConfiguration: async (runner: string) => previewConfiguration(runner),
  saveRunnerConfiguration: async (configuration: import('./contracts/agents').RunnerConfiguration) => savePreviewConfiguration(configuration),
  listAgentRunners: async () => previewRunners(),
  getRunnerModels: async (runner: string, _refresh = false) => previewModels(runner),
  setRunnerKey: desktopRequired,
  getLocalModels: async () => previewLocal(),
  getLocalPulls: async () => [],
  installLocalRunner: desktopRequired,
  startLocalRunner: desktopRequired,
  pullLocalModel: desktopRequired,
  removeLocalModel: desktopRequired,
  selectRunner: async (agent: string, model: string, customBin: string) => {
    rememberPreviewModel(mockAgent, mockModel);
    mockAgent = agent as AppStateView['agent']; mockModel = model; mockCustomBin = customBin;
    rememberPreviewModel(agent, model);
  },
  getOpenrouterFreeOnly: async () => previewFreeOnly,
  setOpenrouterFreeOnly: async (value: boolean) => { previewFreeOnly = value; },
  testAgentConnection: async (agent = 'claude', _customBin = '', model = 'default') => ({ runner: previewRunners().find(r => r.provider === agent || r.id === agent)?.id ?? 'custom-cli' as RunnerId, model, ok: false, detail: 'Connection tests run in the desktop app. This browser preview does not contact providers.', duration_ms: 0 }),
  getAgentActivity: async () => [],
  getAgentPolicy: async (): Promise<AgentPolicy> => JSON.parse(localStorage.getItem('principia.preview.agent-policy') ?? '{"fallback_agent":null,"fallback_model":"default","monthly_budget_usd":0}'),
  setAgentPolicy: async (policy: AgentPolicy) => { localStorage.setItem('principia.preview.agent-policy', JSON.stringify(policy)); },
  completeSetup: async () => {
    setupCompleted = true;
    return appState();
  },
  getCurriculumMap: async (focus: FocusArea): Promise<CurriculumMapView> => {
    const path = previewClassPath(focus as ClassroomSubjectId);
    const plan = path?.recommendation;
    const listed = (list: { id: string }[] | undefined, slug: string) => !!list?.some((topic) => topic.id === slug);
    const statusFor = (slug: string): CurriculumConceptView['path_status'] =>
      !plan ? 'upcoming'
      : listed(plan.bridges, slug) ? 'bridge'
      : listed(plan.checked, slug) ? 'prior_knowledge_checked'
      : listed(plan.bypassed, slug) ? 'bypassed_by_choice'
      : listed(plan.refreshers, slug) ? 'needs_refresher'
      : listed(plan.earlier_topics, slug) ? (plan.earlier_topics.find((t) => t.id === slug)?.reason.startsWith('Declared familiar') ? 'bypassed_by_choice' : 'not_assessed')
      : 'upcoming';
    const concepts = seedConcepts.filter((concept) => concept.focus === focus).map((concept): CurriculumConceptView => ({
      id: seedConcepts.indexOf(concept) + 1,
      slug: concept.slug,
      title: concept.title,
      category: concept.category,
      tier: concept.tier,
      phase: concept.curriculum.phase as CurriculumMapView['current_phase'],
      core: concept.curriculum.core,
      prerequisites: concept.prereqs,
      mastery_state: 'unseen',
      times_picked: 0,
      last_picked_date: null,
      learner_outcome: concept.curriculum.learner_outcome,
      artifact: concept.curriculum.artifact,
      related_concepts: concept.curriculum.related_concepts,
      path_status: statusFor(concept.slug),
      required: concept.curriculum.core && ['upcoming', 'in_progress', 'bridge'].includes(statusFor(concept.slug)),
    }));
    const core = concepts.filter((concept) => concept.core);
    return {
      focus,
      label: CLASSROOM_CATALOG.find((item) => item.id === focus)?.label ?? focus,
      month_outcome: courseDefinition(focus)!.outcome,
      completed_sessions: 0,
      current_phase: 'foundations',
      concepts,
      path: path && plan ? {
        revision: path.revision,
        entry_label: plan.entry_label,
        required_total: core.filter((concept) => concept.required).length,
        required_done: 0,
        coverage_total: core.length,
        coverage_done: 0,
        bypassed: concepts.filter((concept) => concept.path_status === 'bypassed_by_choice').length,
        checked: concepts.filter((concept) => concept.path_status === 'prior_knowledge_checked').length,
        refreshers: concepts.filter((concept) => concept.path_status === 'needs_refresher').length,
        bridges: concepts.filter((concept) => concept.path_status === 'bridge').length,
      } : null,
      bridge_proposals: [],
    };
  },
  configureClassroomProgram: async (input: {
    subject_id: ClassroomSubjectId;
    enabled: boolean;
    agent: AppStateView['agent'];
    model: AppStateView['model'];
    custom_agent_bin: string;
    session_minutes: number;
    start_level?: CefrLevel | null;
    target_level?: CefrLevel | null;
    weekly_minutes?: number | null;
  }) => {
    const settings = mockClassroomSettings[input.subject_id];
    if (input.enabled && !mockClassroomSlots.some(slot => slot.subject_id === input.subject_id && slot.enabled)) throw new Error("Add a study time in this class's Schedule tab before activating it.");
    if (input.enabled && (!settings.enabled || input.session_minutes > settings.sessionMinutes)) {
      const own = mockClassroomSlots.filter((slot) => slot.subject_id === input.subject_id && slot.enabled);
      rejectMockConflicts(own.map((slot) => ({ subject_id: input.subject_id, hour: slot.hour, minute: slot.minute, weekdays: slot.weekdays, session_minutes: input.session_minutes })), { excludedSlotIds: own.map((slot) => slot.id) });
    }
    settings.enabled = input.enabled;
    settings.agent = input.agent;
    settings.model = input.model;
    settings.customBin = input.custom_agent_bin;
    settings.sessionMinutes = input.session_minutes;
    if (input.subject_id === 'german' || input.subject_id === 'italian') {
      const language = mockLanguageSettings[input.subject_id];
      language.enabled = input.enabled;
      language.sessionMinutes = input.session_minutes;
      if (input.start_level) language.startLevel = input.start_level;
      if (input.target_level) language.targetLevel = input.target_level;
      if (input.weekly_minutes) language.weeklyMinutes = input.weekly_minutes;
    }
    return mockClassroomProgram(input.subject_id);
  },
  upsertClassroomSlot: async (input: {
    id?: number | null;
    subject_id: ClassroomSubjectId;
    hour: number;
    minute: number;
    weekdays: number[];
    enabled: boolean;
    durations?: Record<string, number>;
  }) => {
    const id = input.id ?? Math.max(800, ...mockClassroomSlots.map((slot) => slot.id)) + 1;
    const catalog = CLASSROOM_CATALOG.find((item) => item.id === input.subject_id)!;
    const slot: ClassroomSlotView = {
      id,
      subject_id: input.subject_id,
      label: catalog.label,
      short_code: catalog.short,
      kind: catalog.kind,
      durations: Object.fromEntries(Object.entries(input.durations ?? {}).filter(([day]) => input.weekdays.includes(Number(day)))),
      hour: input.hour,
      minute: input.minute,
      weekdays: input.weekdays,
      enabled: input.enabled,
      owed: false,
      in_progress: false,
      next_fire_at: new Date(Date.now() + 8 * 60 * 60 * 1000).toISOString().slice(0, 19),
      source: 'manual',
      occurrence_id: null,
      disposition: null,
    };
    const existing = mockClassroomSlots.findIndex((candidate) => candidate.id === id);
    if (input.id != null && (existing < 0 || mockClassroomSlots[existing].subject_id !== input.subject_id)) throw new Error('classroom slot was not found');
    if (input.enabled) rejectMockConflicts([{ subject_id: input.subject_id, hour: input.hour, minute: input.minute, weekdays: input.weekdays, session_minutes: mockClassroomSettings[input.subject_id].sessionMinutes }], { excludedSlotIds: input.id != null ? [input.id] : [] });
    if (existing >= 0) mockClassroomSlots[existing] = slot;
    else mockClassroomSlots = [...mockClassroomSlots, slot];
    pauseMockClassWithoutSchedule(input.subject_id);
    return mockClassroomSlots;
  },
  planClassroomSchedule: async (input: {
    subject_id: ClassroomSubjectId;
    learning_goal: string;
    target_weekly_minutes: number;
    windows: {
      weekdays: number[];
      start_hour: number;
      start_minute: number;
      end_hour: number;
      end_minute: number;
    }[];
    commit: boolean;
  }): Promise<ClassroomPlanView> => {
    if (input.windows.length === 0) {
      throw new Error('add at least one availability window');
    }
    const catalog = CLASSROOM_CATALOG.find((item) => item.id === input.subject_id)!;
    const settings = mockClassroomSettings[input.subject_id];
    const slots: PlannedSlot[] = [];
    for (const window of input.windows) {
      const start = window.start_hour * 60 + window.start_minute;
      const end = window.end_hour * 60 + window.end_minute;
      if (start >= end) {
        throw new Error('availability window end time must be after its start time');
      }
      const weekdays = Array.from(new Set(window.weekdays)).sort((a, b) => a - b);
      if (weekdays.length === 0) {
        throw new Error('every availability window needs at least one valid weekday');
      }
      const existing = slots.find(
        (slot) => slot.hour === window.start_hour && slot.minute === window.start_minute,
      );
      if (existing) {
        existing.weekdays = Array.from(new Set([...existing.weekdays, ...weekdays])).sort(
          (a, b) => a - b,
        );
      } else {
        slots.push({ hour: window.start_hour, minute: window.start_minute, weekdays });
      }
    }
    slots.sort((a, b) => a.hour - b.hour || a.minute - b.minute);
    const totalWeeklyMinutes = slots.reduce(
      (sum, slot) => sum + slot.weekdays.length * settings.sessionMinutes,
      0,
    );
    const meetsTarget = input.target_weekly_minutes === 0 || totalWeeklyMinutes >= input.target_weekly_minutes;
    const conflicts = scheduleConflicts(slots.map((slot) => ({ subject_id: input.subject_id, hour: slot.hour, minute: slot.minute, weekdays: slot.weekdays, session_minutes: settings.sessionMinutes })), mockClassroomSlots, mockPrograms(), { excludePlannedFor: input.subject_id });
    if (input.commit && conflicts.length) throw new Error(conflictMessage(conflicts));
    if (!input.commit) {
      return {
        slots,
        total_weekly_minutes: totalWeeklyMinutes,
        target_weekly_minutes: input.target_weekly_minutes,
        meets_target: meetsTarget,
        conflicts,
        program: null,
        schedule: null,
      };
    }
    settings.learningGoal = input.learning_goal.trim();
    settings.targetWeeklyMinutes = input.target_weekly_minutes;
    if (input.subject_id === 'german' || input.subject_id === 'italian') {
      mockLanguageSettings[input.subject_id].weeklyMinutes = input.target_weekly_minutes;
    }
    mockClassroomSlots = [
      ...mockClassroomSlots.filter(
        (slot) => !(slot.subject_id === input.subject_id && slot.source === 'planned'),
      ),
      ...slots.map((slot, index) => ({
        id: 900 + index + Math.floor(Math.random() * 100),
        subject_id: input.subject_id,
        label: catalog.label,
        short_code: catalog.short,
        kind: catalog.kind,
        durations: {},
        hour: slot.hour,
        minute: slot.minute,
        weekdays: slot.weekdays,
        enabled: true,
        owed: false,
        in_progress: false,
        next_fire_at: new Date(Date.now() + 8 * 60 * 60 * 1000).toISOString().slice(0, 19),
        source: 'planned' as const,
        occurrence_id: null,
        disposition: null,
      })),
    ];
    return {
      slots,
      total_weekly_minutes: totalWeeklyMinutes,
      target_weekly_minutes: input.target_weekly_minutes,
      meets_target: meetsTarget,
      conflicts,
      program: mockClassroomProgram(input.subject_id),
      schedule: mockClassroomSlots.filter((slot) => slot.subject_id === input.subject_id),
    };
  },
  deleteClassroomSlot: async (id: number) => {
    const removed = mockClassroomSlots.find(slot => slot.id === id);
    mockClassroomSlots = mockClassroomSlots.filter((slot) => slot.id !== id);
    if (removed) pauseMockClassWithoutSchedule(removed.subject_id);
    return mockClassroomSlots;
  },
  startClassroomSession: async (
    subjectId: ClassroomSubjectId,
  ): Promise<ClassroomSessionStart> => {
    if (!mockClassroomProgram(subjectId).enabled) throw new Error("Add a study time and activate this class before starting it.");
    if (subjectId === 'german' || subjectId === 'italian') {
      mockLanguageSessionId += 1;
      mockActiveLanguage = mockLanguageLesson(subjectId);
      mockActiveLanguage.session_id = String(mockLanguageSessionId);
      return { kind: 'language', lesson: mockActiveLanguage };
    }
    mockEngineeringSessionId += 1;
    mockActiveEngineering = mockEngineeringLesson(subjectId);
    mockActiveEngineering.session_id = String(mockEngineeringSessionId);
    return { kind: 'engineering', lesson: mockActiveEngineering };
  },
  startClassReview: async () => { throw new Error('Reviews need the desktop app.'); },
  resumeClassroomSession: async (
    subjectId: ClassroomSubjectId,
  ): Promise<ClassroomSessionStart | null> => {
    if (
      (subjectId === 'german' || subjectId === 'italian') &&
      mockActiveLanguage?.language === subjectId
    ) {
      return { kind: 'language', lesson: mockActiveLanguage };
    }
    if (mockActiveEngineering?.subject_id === subjectId) {
      return { kind: 'engineering', lesson: mockActiveEngineering };
    }
    return null;
  },
  submitClassroomEngineeringSession: async (input: {
    session_id: number;
    answers: number[];
    reflection: string;
  }): Promise<EngineeringSessionResult> => {
    if (!mockActiveEngineering || mockActiveEngineering.session_id !== String(input.session_id)) {
      throw new Error('engineering classroom session not found');
    }
    const lesson = mockActiveEngineering;
    const reference = referenceFor(lesson.subject_id).questions.filter((question) => question.kind === 'mcq');
    if (input.answers.length !== lesson.questions.length || input.answers.some((answer, index) => !Number.isInteger(answer) || answer < 0 || answer >= lesson.questions[index].choices.length)) {
      throw new Error('Answer every displayed question before submitting.');
    }
    const correct = input.answers.map((answer, index) => lesson.questions[index].choices[answer] === reference[index].correct_answer);
    const score = correct.filter(Boolean).length / lesson.questions.length;
    mockActiveEngineering = null;
    return {
      session_id: String(input.session_id),
      subject_id: lesson.subject_id,
      passed: score >= 0.8,
      score,
      corrections: lesson.questions.map((question, index) => ({
        question_id: question.id,
        prompt: question.prompt,
        selected_answer: question.choices[input.answers[index]] ?? '',
        correct_answer: reference[index].correct_answer,
        correct: correct[index],
        explanation: reference[index].explanation,
      })),
      kind: 'lesson' as const,
      fresh_sample: true,
    };
  },
  // Shared-runtime lessons are prepared natively; the browser preview keeps its bundled lessons.
  saveClassLessonWork: async () => { throw new Error('Saved lesson work needs the desktop app.'); },
  saveClassCheckAnswer: async () => { throw new Error('Saved lesson work needs the desktop app.'); },
  submitClassCheck: async () => { throw new Error('Shared-runtime knowledge checks need the desktop app.'); },
  submitClassLanguageCheck: async () => { throw new Error('Shared-runtime knowledge checks need the desktop app.'); },
  skipAppointment: async () => { throw new Error('Appointments need the desktop app.'); },
  rescheduleAppointment: async () => { throw new Error('Appointments need the desktop app.'); },
  setClassFocusPolicy: async (subjectId: ClassroomSubjectId, policy: FocusPolicy) => {
    mockClassroomSettings[subjectId].focusPolicy = policy;
    return mockPrograms().find((program) => program.subject_id === subjectId)!;
  },
  getClassAppointments: async () => [],
  pauseClassLesson: async () => {},
  skipClassLesson: async (sessionId: string) => { if (mockActiveEngineering?.session_id === sessionId) mockActiveEngineering = null; },
  getExercise: async (owner: {
    course_id?: number | null;
    classroom_session_id?: number | null;
  }): Promise<ExerciseView | null> => {
    if (owner.classroom_session_id != null) {
      const lesson =
        mockActiveEngineering?.session_id === String(owner.classroom_session_id)
          ? mockActiveEngineering
          : null;
      if (!lesson?.exercise) return null;
      const completion = mockClassroomExerciseCompletions.get(owner.classroom_session_id);
      return {
        course_id: null,
        classroom_session_id: owner.classroom_session_id,
        study_session_id: null,
        ...lesson.exercise,
        draft: mockClassroomExerciseDrafts.get(owner.classroom_session_id) ?? null,
        completed: completion?.completed ?? false,
        reflection: completion?.reflection ?? '',
      };
    }
    return {
      ...MOCK_EXERCISE,
      course_id: owner.course_id ?? null,
      classroom_session_id: null,
      draft: mockExerciseDraft,
      completed: mockExerciseCompleted,
      reflection: mockExerciseReflection,
    };
  },
  saveExerciseDraft: async (
    owner: { course_id?: number | null; classroom_session_id?: number | null },
    draft: string,
  ) => {
    if (owner.classroom_session_id != null) {
      mockClassroomExerciseDrafts.set(owner.classroom_session_id, draft);
    } else if (owner.course_id === MOCK_COURSE_ID) {
      mockExerciseDraft = draft;
    }
  },
  saveExerciseCompletion: async (
    owner: { course_id?: number | null; classroom_session_id?: number | null },
    completed: boolean,
    reflection: string,
  ) => {
    if (owner.classroom_session_id != null) {
      mockClassroomExerciseCompletions.set(owner.classroom_session_id, {
        completed,
        reflection,
      });
    } else if (owner.course_id === MOCK_COURSE_ID) {
      mockExerciseCompleted = completed;
      mockExerciseReflection = reflection;
    }
  },
  getChat: async (owner: {
    course_id?: number | null;
    classroom_session_id?: number | null;
  }): Promise<ChatMessage[]> => {
    if (owner.classroom_session_id != null) {
      return mockClassroomChatThreads.get(owner.classroom_session_id) ?? [];
    }
    return mockChatThreads.get(owner.course_id ?? -1) ?? [];
  },
  sendChatMessage: async (
    owner: { course_id?: number | null; classroom_session_id?: number | null },
    message: string,
  ): Promise<ChatMessage[]> => {
    const trimmed = message.trim();
    if (!trimmed) throw new Error('message cannot be empty');
    if (trimmed.length > 2_000) throw new Error('message is too long');
    await new Promise((resolve) => setTimeout(resolve, 500));
    if (owner.classroom_session_id != null) {
      const thread = mockClassroomChatThreads.get(owner.classroom_session_id) ?? [];
      thread.push({ role: 'user', content: trimmed, section: null, follow_ups: [] });
      thread.push({
        role: 'assistant',
        content:
          'Start from the smallest mechanism in this lesson, then trace how it composes into the architecture decision. The opening analogy gives you the shape; its “where the analogy breaks” paragraph tells you which runtime constraint must replace the metaphor.',
        section: 'The precise model',
        follow_ups: [
          'Can you trace the mechanism step by step?',
          'Which observation would disprove your current model?',
          'How will you expose this in the exercise?',
        ],
      });
      mockClassroomChatThreads.set(owner.classroom_session_id, thread);
      return thread;
    }
    const thread = mockChatThreads.get(owner.course_id ?? -1) ?? [];
    thread.push({ role: 'user', content: trimmed, section: null, follow_ups: [] });
    thread.push({
      role: 'assistant',
      content:
        "The course's mental model: the loop drains every pending microtask completely before it ever looks at the next macrotask. Think of it like a chef who finishes every add-on ticket for the current dish before glancing at the next order — that ordering is what the diagram in \"The simple version\" is showing.",
      section: 'The precise model',
      follow_ups: [
        'Can you trace one complete event-loop turn?',
        'Which observation distinguishes microtasks from tasks?',
        'How does the exercise reveal checkpoint ordering?',
      ],
    });
    mockChatThreads.set(owner.course_id ?? -1, thread);
    return thread;
  },
  submitLanguageSession: async (input: {
    session_id: number;
    answers: number[];
    writing_response: string;
    speaking_completed: boolean;
    listened: boolean;
    confidence: number;
  }): Promise<LanguageSessionResult> => {
    if (!mockActiveLanguage || String(input.session_id) !== mockActiveLanguage.session_id) {
      throw new Error('language session not found');
    }
    const correctIndexes = [0, 0, 0, 0, 0];
    const correct = input.answers.filter((answer, index) => answer === correctIndexes[index]).length;
    const score = correct / correctIndexes.length;
    const result: LanguageSessionResult = {
      session_id: String(input.session_id),
      passed: score >= 0.6,
      score,
      corrections: mockActiveLanguage.questions.map((question, index) => ({
        question_id: question.id,
        prompt: question.prompt,
        selected_answer: question.choices[input.answers[index]] ?? '',
        correct_answer: question.choices[correctIndexes[index]],
        correct: input.answers[index] === correctIndexes[index],
        explanation:
          input.answers[index] === correctIndexes[index]
            ? 'Correct: this expression fits the scenario and register.'
            : 'Review the model dialogue and retrieve the complete phrase as one chunk.',
      })),
      requirement: null,
      level_advanced_to: null,
      current_level: mockActiveLanguage.level,
      progress: mockLanguageProgram(mockActiveLanguage.language),
    };
    mockActiveLanguage = null;
    return result;
  },
  setKioskLevel: async (level: string) => {
    mockKioskLevel = level as AppStateView['kiosk_level'];
  },
  setModel: async (model: string) => {
    mockModel = model as AppStateView['model'];
  },
  setAgent: async (agent: string, customBin?: string) => {
    mockAgent = agent as AppStateView['agent'];
    mockCustomBin = customBin ?? '';
  },
  setDeepseekApiKey: async (key: string) => {
    mockDeepseekKeyConfigured = key.trim().length > 0;
  },
  snoozeAlarm: async (_occurrenceId: string, minutes: number) => new Date(Date.now() + minutes * 60_000).toISOString(),
  showDesk: async () => {},
  startFromTray: async () => {},
  quitDesk: async () => { throw new Error('The browser preview cannot quit the desk.'); },
  hideTrayPanel: async () => {},
  sizeTrayPanel: async () => {},
  pauseSchedule: async () => {
    mockPaused = true;
  },
  resumeSchedule: async () => {
    mockPaused = false;
  },
  escapeSession: async () => {
    clearMockChatThreads();
    return true;
  },
  getEscapePhrase: async () =>
    'I am choosing to skip my training today and I accept the broken streak',
  getDashboard: async (query: ProgressQuery = {}): Promise<DashboardView> => {
    const programs = CLASSROOM_CATALOG.map(item => mockClassroomProgram(item.id));
    const dateAt = (ago: number) => { const d = new Date(); d.setDate(d.getDate()-ago); return `${d.getFullYear()}-${String(d.getMonth()+1).padStart(2,'0')}-${String(d.getDate()).padStart(2,'0')}`; };
    const history: ProgressEntry[] = Array.from({length: 48}, (_, i) => {
      const program = programs[i % programs.length];
      return { source: program.kind === 'language' ? 'language' : 'classroom', owner_id: String(i+1), date: dateAt(Math.floor(i/2)), subject_id: program.subject_id, title: `${program.label}: ${['Foundations and first principles','Practice and retrieval','Applying the next concept'][i%3]}`, status: i===0 ? 'in_progress' : i%11===0 ? 'skipped' : 'completed', score: i===0 || i%11===0 ? null : 0.7+(i%4)/10, can_read: true };
    });
    const scoped = history.filter(h => !query.subject_id || h.subject_id===query.subject_id);
    const completed = scoped.filter(h => h.status==='completed');
    const matching = scoped.filter(h => (!query.status || h.status===query.status) && `${h.title} ${h.subject_id}`.toLowerCase().includes((query.search??'').trim().toLowerCase()));
    const page = Math.min(Math.max(0,query.page??0),Math.floor(Math.max(0,matching.length-1)/8));
    const dates = new Set(completed.map(h=>h.date));
    let streak = 0; let ago = dates.has(dateAt(0)) ? 0 : 1; while(dates.has(dateAt(ago++))) streak++;
    return {today:dateAt(0), classes:programs.map(p=>({...p,completed_sessions:history.filter(h=>h.subject_id===p.subject_id && h.status==='completed').length})), completed_sessions:completed.length, study_days:dates.size, streak, activity:Array.from({length:28},(_,i)=>({date:dateAt(27-i),completed:completed.filter(h=>h.date===dateAt(27-i)).length})), history:matching.slice(page*8,page*8+8),history_total:matching.length,page,page_size:8};
  },
  getProgressLesson: async (_source: ProgressEntry['source'], _ownerId: string): Promise<ProgressLesson | null> => ({ title: 'Preview lesson', date: new Date().toISOString().slice(0,10), markdown: COURSE_MD, course_id: null, classroom_session_id: null, study_session_id: null }),
  markFrontendReady: async () => {},
};
