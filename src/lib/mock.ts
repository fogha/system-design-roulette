import { previewEnrollmentOptions, previewEnrollmentDraft, savePreviewEnrollmentDraft } from './enrollment-preview';
import type { SaveEnrollmentDraft } from './contracts/enrollment';
import { COURSES, courseDefinition } from './catalog';
import seedConcepts from '../../src-tauri/seed/concepts.json';
/**
 * Demo-mode API: used automatically when the frontend runs outside Tauri
 * (plain `vite dev` in a browser). Lets you develop and screenshot every
 * screen without the Rust backend. Same shapes as ipc.ts.
 */
import type {
  AppStateView,
  ArchivedCourse,
  ChatMessage,
  CourseView,
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
  QuizQuestionView,
  ReviewData,
  RouletteView,
  SessionView,
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

const RESOURCES = [
  {
    title: 'HTML Standard — event loops',
    url: 'https://html.spec.whatwg.org/multipage/webappapis.html#event-loops',
    type: 'spec',
    why: 'Normative definition of macrotasks, microtasks, and rendering steps.',
  },
  {
    title: 'Tasks, microtasks, queues and schedules',
    url: 'https://jakearchibald.com/2015/tasks-microtasks-queues-and-schedules/',
    type: 'article',
    why: 'Classic walkthrough of browser scheduling with runnable examples.',
  },
  {
    title: 'Node.js event loop documentation',
    url: 'https://nodejs.org/en/docs/guides/event-loop-timers-and-nexttick',
    type: 'docs',
    why: 'How libuv phases map onto the same mental model in Node.',
  },
];

const MOCK_COURSE_ID = 42;

const MOCK_CURRICULUM: CurriculumBrief = {
  phase: 'mechanisms',
  core: true,
  learner_outcome:
    'Diagnose browser scheduling behavior in production and defend a mitigation with measured runtime evidence.',
  mechanisms: [
    'task queues and microtask checkpoints',
    'rendering opportunities and main-thread contention',
  ],
  production_scenario:
    'Trace a slow interaction through scheduling, rendering, telemetry, and an explicit rollback decision.',
  misconceptions: ['Promises do not automatically yield to a browser rendering opportunity.'],
  evidence: 'A reproducible trace and before-and-after responsiveness measurement.',
  artifact: 'Extend the browser performance case study with a measured scheduling intervention.',
  primary_sources: ['https://html.spec.whatwg.org/', 'https://developer.mozilla.org/'],
  related_concepts: ['fa-performance-budgets'],
};

const MOCK_EXERCISE: Omit<ExerciseView, 'draft'> = {
  course_id: MOCK_COURSE_ID,
  classroom_session_id: null,
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

const state = {
  step: (skippedPreview ? 'done' : (jump ?? 'quiz')) as SessionView['step'],
  status: (skippedPreview
    ? 'skipped'
    : jump === 'done'
    ? 'completed'
    : jump
      ? 'in_progress'
      : 'pending') as SessionView['status'],
  score: jump ? 2 / 3 : (null as number | null),
  remaining: 30,
  timerId: 0 as ReturnType<typeof setInterval> | 0,
  voluntary: false,
};

let mockExitRound = 1;
let mockExitCount = 5;
let mockExitQuestions: {
  id: number;
  prompt: string;
  choices: string[];
  section: string;
  learning_objective: string;
}[] = [];

const MOCK_EXIT_PROMPTS: { prompt: string; section: string; learning_objective: string }[] = [
  {
    prompt: 'Which queue runs before the next macrotask after synchronous code completes?',
    section: 'Core mechanics',
    learning_objective: 'microtasks drain before the next macrotask',
  },
  {
    prompt: 'How does `await` resume an async function after its operand settles?',
    section: 'Mental model',
    learning_objective: 'await continuations are microtasks',
  },
  {
    prompt: 'What is the main risk of an unbounded `queueMicrotask` chain?',
    section: 'Trade-offs and failure modes',
    learning_objective: 'recursive microtasks can starve rendering',
  },
  {
    prompt: 'What actually blocks JavaScript’s event loop?',
    section: 'Core mechanics',
    learning_objective: 'synchronous CPU work blocks the stack, not promises',
  },
  {
    prompt: 'When can the browser render relative to task and microtask processing?',
    section: 'The simple version',
    learning_objective: 'rendering happens after microtasks drain',
  },
  {
    prompt: 'Why can `setTimeout(fn, 0)` still run noticeably later?',
    section: 'Core mechanics',
    learning_objective: 'macrotasks wait for the full microtask drain first',
  },
  {
    prompt: 'Which ordering follows sync code, a resolved Promise, and `setTimeout(0)`?',
    section: 'Core mechanics',
    learning_objective: 'sync, then microtasks, then macrotasks',
  },
  {
    prompt: 'What does a host API do when its asynchronous work completes?',
    section: 'Core mechanics',
    learning_objective: 'host APIs enqueue macrotasks on completion',
  },
  {
    prompt: 'Why does `await` not move synchronous CPU work off the main thread?',
    section: 'Trade-offs and failure modes',
    learning_objective: 'await only defers scheduling, not computation',
  },
  {
    prompt: 'Which experiment best reveals microtask starvation?',
    section: 'Runnable experiment',
    learning_objective: 'recursive .then() chains starve macrotasks',
  },
];

function session(): SessionView {
  return {
    date: new Date().toISOString().slice(0, 10),
    status: state.status,
    step: state.step,
    quiz_score: state.score,
    streak: 17,
    // &unlocked previews voluntary (early-start/extension) sessions.
    locked: state.status === 'in_progress' && !params.has('unlocked') && !state.voluntary,
    // ?type=pop_quiz previews an audit day in the browser demo.
    session_type: params.get('type') === 'pop_quiz' ? 'pop_quiz' : 'lesson',
    plan_reason:
      params.get('type') === 'pop_quiz' ? 'review debt: 4 topics due — surprise audit' : '',
    focus: mockSelectedFocus,
  };
}

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
        weekdays: [1, 2, 3, 4, 5, 6],
        enabled: true,
        owed: params.has('classDue') || params.has('languageDue'),
        next_fire_at: new Date(Date.now() + 4 * 60 * 60 * 1000).toISOString().slice(0, 19),
        in_progress: false,
        source: 'manual',
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
    session_id: mockLanguageSessionId,
    language,
    label: german ? 'German' : 'Italian',
    native_label: target,
    level: 'A1',
    unit_slug: german ? 'de-a1-greetings-introductions' : 'it-a1-greetings',
    phase: 1,
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

function mockClassroomProgram(subjectId: ClassroomSubjectId): ClassroomProgramView {
  const catalog = CLASSROOM_CATALOG.find((item) => item.id === subjectId)!;
  const settings = mockClassroomSettings[subjectId];
  const languageProgress =
    catalog.kind === 'language' ? mockLanguageProgram(subjectId as LanguageId) : null;
  return {
    subject_id: subjectId,
    kind: catalog.kind,
    label: catalog.label,
    native_label: catalog.native,
    short_code: catalog.short,
    enabled: settings.enabled,
    agent: settings.agent,
    model: settings.model,
    custom_agent_bin: settings.customBin,
    prompt_profile: `classroom.${subjectId}`,
    prompt_version: 'v1',
    session_minutes: settings.sessionMinutes,
    learning_goal: settings.learningGoal,
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
    session_id: mockEngineeringSessionId,
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
    agent_used: mockClassroomSettings[subjectId].agent,
    prompt_profile: `classroom.${subjectId}`,
    prompt_version: `classroom.${subjectId}.v1`,
    estimated_minutes: mockClassroomSettings[subjectId].sessionMinutes,
    status: 'in_progress',
  };
}

function appState(): AppStateView {
  const inSetup = location.search.includes('setup') && !setupCompleted;
  return {
    onboarded: !inSetup,
    session: session(),
    selected_focus: mockSelectedFocus,
    // After completing setup the session is not owed yet (scheduled time is
    // in the future) — mirrors the real backend so routing bugs reproduce.
    owed: !setupCompleted && !params.has('unlocked'),
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
    classroom_programs: CLASSROOM_CATALOG.map((item) => mockClassroomProgram(item.id)),
    classroom_slots: mockClassroomSlots,
    classroom_due_count: mockClassroomSlots.filter((slot) => slot.owed).length,
    active_classroom_sessions: [
      ...(mockActiveLanguage
        ? [
            {
              session_id: mockActiveLanguage.session_id,
              subject_id: mockActiveLanguage.language,
              kind: 'language' as const,
              label: mockActiveLanguage.label,
              title: mockActiveLanguage.title,
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
            },
          ]
        : []),
    ],
  };
}

const QUESTIONS: QuizQuestionView[] = [
  {
    id: 1,
    prompt:
      'What is the guaranteed console order for this snippet?\n\n```js\nconsole.log("A");\nsetTimeout(() => console.log("B"), 0);\nPromise.resolve().then(() => console.log("C"));\n```',
    kind: 'mcq',
    choices: ['A, B, C', 'A, C, B', 'C, A, B', 'B, C, A'],
    origin: 'carryover',
    answered: false,
    draft: null,
  },
  {
    id: 2,
    prompt: 'Why can an infinite chain of Promise.then callbacks prevent setTimeout from firing?',
    kind: 'mcq',
    choices: [
      'Microtasks drain completely before the next macrotask, starving the timer queue',
      'Promises run on a separate thread that blocks the timer thread',
      'setTimeout(0) is coalesced into the same microtask turn',
      'The call stack cannot unwind until all promises settle',
    ],
    origin: 'fresh',
    answered: false,
    draft: null,
  },
  {
    id: 3,
    prompt:
      'An async function awaits a resolved promise, logs "after", and the caller logs "caller" immediately after invoking it. Explain the ordering and which queue resumes the async function.',
    kind: 'free',
    choices: null,
    origin: 'fresh',
    answered: false,
    draft: null,
  },
];

const REVIEW: ReviewData = {
  score: 2 / 3,
  self_assess: false,
  items: [
    {
      question_id: 1,
      prompt: QUESTIONS[0].prompt,
      kind: 'mcq',
      user_answer: 'A, C, B',
      correct: true,
      feedback: '',
      correct_answer: 'A, C, B',
      explanation:
        'Synchronous A runs first; microtask C runs before the next macrotask B — the canonical event-loop ordering check.',
      returns_tomorrow: false,
    },
    {
      question_id: 3,
      prompt: QUESTIONS[2].prompt,
      kind: 'free',
      user_answer: 'The async function runs synchronously until await, so caller logs first.',
      correct: false,
      feedback:
        'Close on the sync portion, but you missed that await schedules the continuation as a microtask after caller logs.',
      correct_answer:
        'The async body runs synchronously until await; caller logs next; then the microtask resumes the async function and logs "after".',
      explanation:
        'await on an already-resolved promise still yields — the continuation is a microtask, not synchronous stack work.',
      returns_tomorrow: true,
    },
  ],
};

export const mockApi = {
  getEnrollmentOptions: async (courseId: ClassroomSubjectId) => previewEnrollmentOptions(courseId),
  getEnrollmentDraft: async (courseId: ClassroomSubjectId) => previewEnrollmentDraft(courseId),
  saveEnrollmentDraft: async (input: SaveEnrollmentDraft) => savePreviewEnrollmentDraft(input),
  getCatalog: async () => [...COURSES],
  getAppState: async () => appState(),
  checkAgent: async () => true,
  completeSetup: async () => {
    setupCompleted = true;
    return appState();
  },
  updateSchedule: async () => {},
  getCurriculumMap: async (focus: FocusArea): Promise<CurriculumMapView> => ({
    focus,
    label: CLASSROOM_CATALOG.find((item) => item.id === focus)?.label ?? focus,
    month_outcome: courseDefinition(focus)!.outcome,
    completed_sessions: 0,
    current_phase: 'foundations',
    concepts: seedConcepts.filter((concept) => concept.focus === focus).map((concept) => ({
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
    })),
  }),
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
    if (!input.enabled) {
      mockClassroomSlots = mockClassroomSlots.filter(
        (slot) => slot.subject_id !== input.subject_id,
      );
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
  }) => {
    const id = input.id ?? Math.max(800, ...mockClassroomSlots.map((slot) => slot.id)) + 1;
    const catalog = CLASSROOM_CATALOG.find((item) => item.id === input.subject_id)!;
    const slot: ClassroomSlotView = {
      id,
      subject_id: input.subject_id,
      label: catalog.label,
      short_code: catalog.short,
      kind: catalog.kind,
      hour: input.hour,
      minute: input.minute,
      weekdays: input.weekdays,
      enabled: input.enabled,
      owed: false,
      in_progress: false,
      next_fire_at: new Date(Date.now() + 8 * 60 * 60 * 1000).toISOString().slice(0, 19),
      source: 'manual',
    };
    const existing = mockClassroomSlots.findIndex((candidate) => candidate.id === id);
    if (existing >= 0) mockClassroomSlots[existing] = slot;
    else mockClassroomSlots = [...mockClassroomSlots, slot];
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
    if (!input.commit) {
      return {
        slots,
        total_weekly_minutes: totalWeeklyMinutes,
        target_weekly_minutes: input.target_weekly_minutes,
        meets_target: meetsTarget,
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
        hour: slot.hour,
        minute: slot.minute,
        weekdays: slot.weekdays,
        enabled: true,
        owed: false,
        in_progress: false,
        next_fire_at: new Date(Date.now() + 8 * 60 * 60 * 1000).toISOString().slice(0, 19),
        source: 'planned' as const,
      })),
    ];
    return {
      slots,
      total_weekly_minutes: totalWeeklyMinutes,
      target_weekly_minutes: input.target_weekly_minutes,
      meets_target: meetsTarget,
      program: mockClassroomProgram(input.subject_id),
      schedule: mockClassroomSlots.filter((slot) => slot.subject_id === input.subject_id),
    };
  },
  deleteClassroomSlot: async (id: number) => {
    mockClassroomSlots = mockClassroomSlots.filter((slot) => slot.id !== id);
    return mockClassroomSlots;
  },
  startClassroomSession: async (
    subjectId: ClassroomSubjectId,
  ): Promise<ClassroomSessionStart> => {
    if (subjectId === 'german' || subjectId === 'italian') {
      mockLanguageSessionId += 1;
      mockActiveLanguage = mockLanguageLesson(subjectId);
      mockActiveLanguage.session_id = mockLanguageSessionId;
      return { kind: 'language', lesson: mockActiveLanguage };
    }
    mockEngineeringSessionId += 1;
    mockActiveEngineering = mockEngineeringLesson(subjectId);
    mockActiveEngineering.session_id = mockEngineeringSessionId;
    return { kind: 'engineering', lesson: mockActiveEngineering };
  },
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
    if (!mockActiveEngineering || mockActiveEngineering.session_id !== input.session_id) {
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
      session_id: input.session_id,
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
    };
  },
  getExercise: async (owner: {
    course_id?: number | null;
    classroom_session_id?: number | null;
  }): Promise<ExerciseView | null> => {
    if (owner.classroom_session_id != null) {
      const lesson =
        mockActiveEngineering?.session_id === owner.classroom_session_id
          ? mockActiveEngineering
          : null;
      if (!lesson?.exercise) return null;
      const completion = mockClassroomExerciseCompletions.get(owner.classroom_session_id);
      return {
        course_id: null,
        classroom_session_id: owner.classroom_session_id,
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
    if (!mockActiveLanguage || input.session_id !== mockActiveLanguage.session_id) {
      throw new Error('language session not found');
    }
    const correctIndexes = [0, 0, 0, 0, 0];
    const correct = input.answers.filter((answer, index) => answer === correctIndexes[index]).length;
    const score = correct / correctIndexes.length;
    const result: LanguageSessionResult = {
      session_id: input.session_id,
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
  pauseSchedule: async () => {
    mockPaused = true;
  },
  resumeSchedule: async () => {
    mockPaused = false;
  },
  startSession: async (focus: FocusArea) => {
    mockSelectedFocus = focus;
    state.status = 'in_progress';
    state.step = 'quiz';
    clearMockChatThreads();
    return session();
  },
  getQuiz: async () => QUESTIONS,
  submitAnswer: async (id: number) => {
    const q = QUESTIONS.find((q) => q.id === id);
    if (q) q.answered = true;
  },
  finishQuiz: async () => {
    state.step = 'review';
    state.score = REVIEW.score;
    return REVIEW;
  },
  getReview: async () => REVIEW,
  finishReview: async () => {
    state.step = 'roulette';
    mockEmit('session:state', session());
    return session();
  },
  completeTrackDay: async () => {
    state.step = 'done';
    state.status = 'completed';
    mockEmit('session:state', session());
    return session();
  },
  getRoulette: async (): Promise<RouletteView> => ({
    pool: [
      'Closures and lexical scope',
      'Prototypes vs classes',
      'The event loop and task queues',
      'V8 hidden classes and inline caches',
      'WeakMap and garbage collection',
      'Proxy and Reflect traps',
      'Structured cloning algorithm',
      'Atomics and SharedArrayBuffer',
      'import() and module graphs',
      'Error stack trace mechanics',
      'Intl and locale-sensitive APIs',
      'Temporal proposal patterns',
    ],
    chosen_index: 2,
    concept_title: 'The event loop and task queues',
    concept_category: 'runtime',
    pool_unlocked: 31,
    pool_total: 72,
    track_complete: false,
  }),
  ensureCourse: async (): Promise<CourseView> => {
    // Demo the live agent log the way a real generation streams it.
    const feed = [
      'spawn: agent · model opus',
      'tool: WebSearch javascript event loop microtasks spec',
      'tool: WebFetch https://html.spec.whatwg.org/multipage/webappapis.html',
      'tool: WebSearch node event loop libuv phases',
      'draft: 2,100 chars written',
      'draft: 8,400 chars written',
      'done: agent returned 19,872 chars',
    ];
    for (const line of feed) {
      mockEmit('gen:log', line);
      await new Promise((r) => setTimeout(r, 350));
    }
    return {
      course_id: MOCK_COURSE_ID,
      title: 'The JavaScript event loop and task queues',
      concept_slug: 'js-event-loop',
      curriculum: MOCK_CURRICULUM,
      prerequisites: [],
      session_index: 6,
      why_now:
        'Session 6 advances the mechanisms phase by turning prior scheduling vocabulary into production diagnosis.',
      markdown: COURSE_MD,
      resources: RESOURCES,
      source: 'claude',
      remaining_seconds: state.remaining,
      total_seconds: 30 * 60,
    };
  },
  startCourse: async () => {
    state.step = 'course';
    state.remaining = 27 * 60 + 14;
    if (!state.timerId) {
      state.timerId = setInterval(() => {
        state.remaining = Math.max(0, state.remaining - 1);
        mockEmit('timer:tick', state.remaining);
      }, 1000);
    }
    return session();
  },
  finishCourse: async () => {
    state.step = 'done';
    state.status = 'completed';
    clearMockChatThreads();
    mockEmit('session:state', session());
    return session();
  },
  escapeSession: async () => {
    clearMockChatThreads();
    return true;
  },
  getEscapePhrase: async () =>
    'I am choosing to skip my training today and I accept the broken streak',
  getDashboard: async (): Promise<DashboardView> => {
    const topics = [
      'The event loop and microtask checkpoints',
      'Closures, scope, and the lexical environment',
      'V8 hidden classes and shape transitions',
      'Prototypes, delegation, and property lookup',
      'Promise internals and async/await lowering',
      'WeakRef, FinalizationRegistry, and GC edges',
      'Proxy traps and invariant semantics',
      'Structured clone and transferables',
      'Module graphs and live bindings',
      'Atomics, workers, and shared memory',
    ];
    const history = [];
    const today = new Date();
    for (let i = 1; i <= 36; i++) {
      const d = new Date(today);
      d.setDate(d.getDate() - i);
      const skipped = i === 9 || i === 23;
      history.push({
        date: d.toISOString().slice(0, 10),
        status: skipped ? 'skipped' : 'completed',
        quiz_score: skipped ? null : Math.round((0.5 + ((i * 37) % 50) / 100) * 100) / 100,
        concept_title: topics[i % topics.length],
      });
    }
    const states = [
      'mastered', 'maintenance', 'practicing', 'practicing', 'struggling',
      'introduced', 'decayed', 'unseen', 'unseen', 'unseen',
    ] as const;
    const categories = ['runtime', 'language', 'memory', 'async', 'modules', 'platform'];
    const mastery = Array.from({ length: 72 }, (_, i) => ({
      concept_id: i + 1,
      slug: `concept-${i + 1}`,
      title: topics[i % topics.length],
      category: categories[Math.floor(i / 12)],
      state: states[(i * 7) % states.length],
      score_ema: ((i * 13) % 100) / 100,
    }));
    return {
      history,
      streak: 17,
      carryover_due: 2,
      concepts_total: 72,
      concepts_covered: 36,
      mastery,
    };
  },
  getPastCourse: async (): Promise<ArchivedCourse | null> => ({
    course_id: MOCK_COURSE_ID,
    session_date: new Date().toISOString().slice(0, 10),
    title: 'The JavaScript event loop and task queues',
    markdown: COURSE_MD,
    resources: RESOURCES,
  }),
  openResources: async () => RESOURCES.length,
  markFrontendReady: async () => {},
  ensureAudio: async () => ({
    engine: 'speech' as const,
    lines: [
      { speaker: 'teacher' as const, text: "Today we're on the event loop — the scheduler every async API shares. Before I explain: what order do you expect from sync code, a zero-delay timer, and a resolved promise?" },
      { speaker: 'student' as const, text: 'Sync first, then the timer, then the promise — because the timer was scheduled first?' },
      { speaker: 'teacher' as const, text: "That's the trap. Microtasks — promise reactions — drain completely before the next macrotask. You'll see sync, then the promise, then the timer." },
      { speaker: 'student' as const, text: 'So await is also a microtask continuation?' },
      { speaker: 'teacher' as const, text: 'Exactly. await suspends the async function and resumes on a microtask when the operand settles — same queue, same starvation rules if you recurse without yielding.' },
      { speaker: 'student' as const, text: 'And blocking the loop is really blocking the call stack with sync CPU work.' },
      { speaker: 'teacher' as const, text: "Now you've got the mental model. Build a tiny log-ordering experiment in the console — that's the executable proof you'll reuse forever." },
    ],
  }),
  getAudioEnabled: async () => false,
  setAudioEnabled: async () => {},
  getExitQuiz: async () => {
    if (mockExitQuestions.length === 0) {
      mockExitQuestions = Array.from({ length: mockExitCount }, (_, index) => {
        const source = MOCK_EXIT_PROMPTS[((mockExitRound - 1) * 5 + index) % MOCK_EXIT_PROMPTS.length];
        return {
          id: mockExitRound * 100 + index,
          prompt: source.prompt,
          section: source.section,
          learning_objective: source.learning_objective,
          choices: [
            'The mechanism described in the course',
            'Whichever callback was registered first',
            'A separate worker always handles it',
            'The runtime chooses randomly',
          ],
        };
      });
    }
    return mockExitQuestions.map(({ id, prompt, choices }) => ({ id, prompt, choices }));
  },
  submitExitQuiz: async (answers: Record<number, string>) => {
    const correctAnswer = 'The mechanism described in the course';
    const correct = mockExitQuestions
      .filter((question) => answers[question.id] === correctAnswer)
      .map((question) => question.id);
    const incorrect = mockExitQuestions
      .filter((question) => answers[question.id] !== correctAnswer)
      .map((question) => ({
        question_id: question.id,
        prompt: question.prompt,
        user_answer: answers[question.id] ?? '',
        correct_answer: correctAnswer,
        explanation:
          'The misconception: assuming this follows intuition rather than the loop\'s actual queue order. The course traces this behavior through the call stack, host scheduling, and the microtask checkpoint — like a chef finishing every add-on ticket before touching the next new order.',
        section: question.section,
        learning_objective: question.learning_objective,
      }));
    const passed = incorrect.length === 0 && correct.length === mockExitQuestions.length;
    const round = mockExitRound;
    const nextQuestionCount = mockExitCount + incorrect.length;
    const nextFocusAreas = incorrect.map((item) =>
      item.section && item.learning_objective
        ? `${item.section} — ${item.learning_objective}`
        : item.learning_objective || item.section || 'the missed question'
    );
    if (passed) {
      state.remaining = 0;
      mockEmit('timer:tick', 0);
      mockEmit('timer:done', true);
    } else {
      mockExitRound += 1;
      mockExitCount = nextQuestionCount;
      mockExitQuestions = [];
    }
    return {
      passed,
      correct,
      incorrect,
      round,
      next_question_count: nextQuestionCount,
      next_focus_areas: nextFocusAreas,
    };
  },
};
