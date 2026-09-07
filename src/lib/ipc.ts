import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

export type FocusArea =
  | 'javascript'
  | 'typescript'
  | 'frontend-architecture'
  | 'developer-tooling';

export type LanguageId = 'german' | 'italian';
export type ClassroomSubjectId = LanguageId | FocusArea;
export type ClassroomKind = 'language' | 'engineering';
export type AgentId = 'claude' | 'codex' | 'cursor' | 'gemini' | 'deepseek' | 'custom';
export type ModelId = 'opus' | 'sonnet' | 'haiku';
export type CefrLevel = 'A1' | 'A2' | 'B1' | 'B2';
export type LanguageStrand =
  | 'listening'
  | 'reading'
  | 'spoken_interaction'
  | 'spoken_production'
  | 'writing'
  | 'grammar'
  | 'vocabulary_pragmatics';

export interface LanguageMilestone {
  level: CefrLevel;
  target_date: string;
  reached: boolean;
}

export interface LanguageSkillScore {
  id: LanguageStrand;
  label: string;
  score: number;
  encounters: number;
}

export interface OfficialLanguageSource {
  title: string;
  url: string;
}

export interface LanguageProgramView {
  language: LanguageId;
  label: string;
  native_label: string;
  enabled: boolean;
  start_level: CefrLevel;
  current_level: CefrLevel;
  target_level: CefrLevel;
  start_date: string;
  target_date: string;
  weekly_minutes: number;
  session_minutes: number;
  completed_steps: number;
  required_steps: number;
  progress: number;
  total_completed_steps: number;
  total_required_steps: number;
  recommended_weekly_minutes: number;
  pace_status: 'on_track' | 'commitment_gap' | 'behind';
  pace_message: string;
  milestones: LanguageMilestone[];
  skills: LanguageSkillScore[];
  official_sources: OfficialLanguageSource[];
}

export interface LanguagePhrase {
  target: string;
  translation: string;
  note: string;
}

export interface LanguageDialogueLine {
  speaker: string;
  target: string;
  translation: string;
}

export interface LanguageQuestionView {
  id: number;
  prompt: string;
  choices: string[];
  strand: LanguageStrand;
}

export interface LanguageLessonView {
  session_id: number;
  language: LanguageId;
  label: string;
  native_label: string;
  level: CefrLevel;
  unit_slug: string;
  phase: number;
  phase_label: string;
  title: string;
  scenario: string;
  can_do: string;
  markdown: string;
  phrases: LanguagePhrase[];
  dialogue: LanguageDialogueLine[];
  questions: LanguageQuestionView[];
  speaking_prompt: string;
  writing_prompt: string;
  listen_text: string;
  estimated_minutes: number;
  status: 'in_progress' | 'completed' | 'skipped';
}

export interface LanguageCorrection {
  question_id: number;
  prompt: string;
  selected_answer: string;
  correct_answer: string;
  correct: boolean;
  explanation: string;
}

export interface LanguageSessionResult {
  session_id: number;
  passed: boolean;
  score: number;
  corrections: LanguageCorrection[];
  level_advanced_to: CefrLevel | null;
  current_level: CefrLevel;
  progress: LanguageProgramView;
}

export interface ClassroomProgramView {
  subject_id: ClassroomSubjectId;
  kind: ClassroomKind;
  label: string;
  native_label: string;
  short_code: string;
  enabled: boolean;
  agent: AgentId;
  model: ModelId;
  custom_agent_bin: string;
  prompt_profile: string;
  prompt_version: string;
  session_minutes: number;
  learning_goal: string;
  target_weekly_minutes: number;
  progress: number;
  progress_label: string;
  completed: boolean;
  language_progress: LanguageProgramView | null;
}

export type CurriculumPhase =
  | 'foundations'
  | 'mechanisms'
  | 'production'
  | 'synthesis'
  | 'elective';

export interface CurriculumBrief {
  phase: CurriculumPhase;
  core: boolean;
  learner_outcome: string;
  mechanisms: string[];
  production_scenario: string;
  misconceptions: string[];
  evidence: string;
  artifact: string;
  primary_sources: string[];
  related_concepts: string[];
}

export interface CurriculumConceptView {
  id: number;
  slug: string;
  title: string;
  category: string;
  tier: number;
  phase: CurriculumPhase;
  core: boolean;
  prerequisites: string[];
  mastery_state: MasteryEntry['state'];
  times_picked: number;
  last_picked_date: string | null;
  learner_outcome: string;
  artifact: string;
  related_concepts: string[];
}

export interface CurriculumMapView {
  focus: FocusArea;
  label: string;
  month_outcome: string;
  completed_sessions: number;
  current_phase: CurriculumPhase;
  concepts: CurriculumConceptView[];
}

export interface ClassroomSlotView {
  id: number;
  subject_id: ClassroomSubjectId;
  label: string;
  short_code: string;
  kind: ClassroomKind;
  hour: number;
  minute: number;
  weekdays: number[];
  enabled: boolean;
  owed: boolean;
  next_fire_at: string;
  in_progress: boolean;
  source: 'manual' | 'planned';
}

export interface AvailabilityWindow {
  weekdays: number[];
  start_hour: number;
  start_minute: number;
  end_hour: number;
  end_minute: number;
}

export interface PlannedSlot {
  hour: number;
  minute: number;
  weekdays: number[];
}

export interface ClassroomPlanView {
  slots: PlannedSlot[];
  total_weekly_minutes: number;
  target_weekly_minutes: number;
  meets_target: boolean;
  program: ClassroomProgramView | null;
  schedule: ClassroomSlotView[] | null;
}

export interface ActiveClassroomSessionView {
  session_id: number;
  subject_id: ClassroomSubjectId;
  kind: ClassroomKind;
  label: string;
  title: string;
}

export interface ClassroomQuestionView {
  id: number;
  prompt: string;
  choices: string[];
  section: string;
  learning_objective: string;
}

export interface ClassroomExercise {
  title: string;
  instructions: string;
  starter_code: string | null;
  deliverable: string | null;
  hints: string[];
}

export interface ClassroomExerciseView extends ClassroomExercise {
  session_id: number;
  draft: string | null;
  completed: boolean;
  reflection: string;
}

export interface EngineeringLessonView {
  session_id: number;
  subject_id: FocusArea;
  label: string;
  short_code: string;
  title: string;
  concept_slug: string;
  concept_title: string;
  category: string;
  curriculum: CurriculumBrief;
  prerequisites: string[];
  session_index: number;
  why_now: string;
  markdown: string;
  resources: Resource[];
  questions: ClassroomQuestionView[];
  exercise: ClassroomExercise | null;
  agent_used: string;
  prompt_profile: string;
  prompt_version: string;
  estimated_minutes: number;
  status: 'in_progress' | 'completed' | 'skipped';
}

export type ClassroomSessionStart =
  | { kind: 'language'; lesson: LanguageLessonView }
  | { kind: 'engineering'; lesson: EngineeringLessonView };

export interface ClassroomCorrection {
  question_id: number;
  prompt: string;
  selected_answer: string;
  correct_answer: string;
  correct: boolean;
  explanation: string;
}

export interface EngineeringSessionResult {
  session_id: number;
  subject_id: FocusArea;
  passed: boolean;
  score: number;
  corrections: ClassroomCorrection[];
}

export interface SessionView {
  date: string;
  status: 'pending' | 'in_progress' | 'completed' | 'skipped';
  step: 'quiz' | 'review' | 'roulette' | 'course' | 'done';
  quiz_score: number | null;
  streak: number;
  locked: boolean;
  session_type: 'lesson' | 'pop_quiz';
  plan_reason: string;
  focus: FocusArea;
}

export interface AppStateView {
  onboarded: boolean;
  session: SessionView;
  selected_focus: FocusArea;
  owed: boolean;
  schedule_hour: number;
  schedule_minute: number;
  debug_day: boolean;
  enforcement_disarmed: boolean;
  schedule_paused: boolean;
  kiosk_level: 'advisory' | 'firm' | 'hard';
  model: ModelId;
  agent: AgentId;
  custom_agent_bin: string;
  deepseek_key_configured: boolean;
  classroom_programs: ClassroomProgramView[];
  classroom_slots: ClassroomSlotView[];
  classroom_due_count: number;
  active_classroom_sessions: ActiveClassroomSessionView[];
}

export interface QuizQuestionView {
  id: number;
  prompt: string;
  kind: 'mcq' | 'free';
  choices: string[] | null;
  origin: 'fresh' | 'carryover';
  answered: boolean;
  draft: string | null;
}

export interface ReviewItem {
  question_id: number;
  prompt: string;
  kind: string;
  user_answer: string;
  correct: boolean | null;
  feedback: string;
  correct_answer: string;
  explanation: string;
  returns_tomorrow: boolean;
}

export interface ReviewData {
  items: ReviewItem[];
  score: number;
  self_assess: boolean;
}

export interface RouletteView {
  pool: string[];
  chosen_index: number;
  concept_title: string;
  concept_category: string;
  pool_unlocked: number;
  pool_total: number;
  track_complete: boolean;
}

export interface Resource {
  title: string;
  url: string;
  type?: string;
  why?: string;
}

export interface CourseView {
  course_id: number;
  title: string;
  concept_slug: string;
  curriculum: CurriculumBrief;
  prerequisites: string[];
  session_index: number;
  why_now: string;
  markdown: string;
  resources: Resource[];
  source: string;
  remaining_seconds: number;
  total_seconds: number;
}

export interface HistoryEntry {
  date: string;
  status: string;
  quiz_score: number | null;
  concept_title: string | null;
}

export interface MasteryEntry {
  concept_id: number;
  slug: string;
  title: string;
  category: string;
  state:
    | 'unseen'
    | 'introduced'
    | 'practicing'
    | 'struggling'
    | 'mastered'
    | 'maintenance'
    | 'decayed';
  score_ema: number;
}

export interface DashboardView {
  history: HistoryEntry[];
  streak: number;
  carryover_due: number;
  concepts_total: number;
  concepts_covered: number;
  mastery: MasteryEntry[];
}

export interface AudioLine {
  speaker: 'teacher' | 'student';
  text: string;
  file?: string | null;
}

export interface AudioView {
  lines: AudioLine[];
  engine: 'speech' | 'vibevoice';
}

export interface ExitQuizQuestion {
  id: number;
  prompt: string;
  choices: string[];
}

export interface ExitQuizFeedback {
  question_id: number;
  prompt: string;
  user_answer: string;
  correct_answer: string;
  explanation: string;
  section: string;
  learning_objective: string;
}

export interface ExitQuizResult {
  passed: boolean;
  correct: number[];
  incorrect: ExitQuizFeedback[];
  round: number;
  next_question_count: number;
  next_focus_areas: string[];
}

/**
 * One turn of the session-only, course-grounded chat. Held only in the
 * Rust process's memory for the active app session — never persisted, and
 * gone on completion, skip, extension, or app restart.
 */
export interface ChatMessage {
  role: 'user' | 'assistant';
  content: string;
  /** Exact course heading that grounds an assistant response. */
  section: string | null;
  /** Three course-grounded prompts for productive follow-up. */
  follow_ups: string[];
}

export interface ArchivedCourse {
  course_id: number;
  session_date: string;
  title: string;
  markdown: string;
  resources: Resource[];
}

/**
 * Structured exercise for a course, with any autosaved draft. Reachable
 * from the active reader and from archived-course history — never gated
 * by session completion.
 */
export interface ExerciseView {
  course_id: number;
  title: string;
  instructions: string;
  starter_code: string | null;
  deliverable: string | null;
  hints: string[];
  draft: string | null;
  completed: boolean;
  reflection: string;
}

export const isTauri =
  typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

const realApi = {
  markFrontendReady: () => invoke<void>('mark_frontend_ready'),
  getAppState: () => invoke<AppStateView>('get_app_state'),
  checkAgent: (agent?: string, customBin?: string) =>
    invoke<boolean>('check_agent', { agent, customBin }),
  completeSetup: (
    hour: number,
    minute: number,
    escapePhrase: string,
    kioskLevel = 'hard',
    model = 'opus',
    agent = 'claude',
    customAgentBin = ''
  ) =>
    invoke<AppStateView>('complete_setup', {
      input: {
        hour,
        minute,
        escape_phrase: escapePhrase,
        kiosk_level: kioskLevel,
        model,
        agent,
        custom_agent_bin: customAgentBin,
      },
    }),
  setKioskLevel: (level: string) => invoke<void>('set_kiosk_level', { level }),
  setModel: (model: string) => invoke<void>('set_model', { model }),
  setAgent: (agent: string, customBin?: string) =>
    invoke<void>('set_agent', { agent, customBin }),
  setDeepseekApiKey: (key: string) => invoke<void>('set_deepseek_api_key', { key }),
  updateSchedule: (hour: number, minute: number) =>
    invoke<void>('update_schedule', { hour, minute }),
  getCurriculumMap: (focus: FocusArea) =>
    invoke<CurriculumMapView>('get_curriculum_map', { focus }),
  configureClassroomProgram: (input: {
    subject_id: ClassroomSubjectId;
    enabled: boolean;
    agent: AgentId;
    model: ModelId;
    custom_agent_bin: string;
    session_minutes: number;
    start_level?: CefrLevel | null;
    target_level?: CefrLevel | null;
    weekly_minutes?: number | null;
  }) => invoke<ClassroomProgramView>('configure_classroom_program', { input }),
  upsertClassroomSlot: (input: {
    id?: number | null;
    subject_id: ClassroomSubjectId;
    hour: number;
    minute: number;
    weekdays: number[];
    enabled: boolean;
  }) => invoke<ClassroomSlotView[]>('upsert_classroom_slot', { input }),
  deleteClassroomSlot: (id: number) =>
    invoke<ClassroomSlotView[]>('delete_classroom_slot', { id }),
  planClassroomSchedule: (input: {
    subject_id: ClassroomSubjectId;
    learning_goal: string;
    target_weekly_minutes: number;
    windows: AvailabilityWindow[];
    commit: boolean;
  }) => invoke<ClassroomPlanView>('plan_classroom_schedule', { input }),
  startClassroomSession: (
    subjectId: ClassroomSubjectId,
    slotId?: number | null,
    revisit?: boolean,
  ) =>
    invoke<ClassroomSessionStart>('start_classroom_session', {
      subjectId,
      slotId: slotId ?? null,
      revisit: revisit ?? false,
    }),
  resumeClassroomSession: (subjectId: ClassroomSubjectId) =>
    invoke<ClassroomSessionStart | null>('resume_classroom_session', { subjectId }),
  submitClassroomEngineeringSession: (input: {
    session_id: number;
    answers: number[];
    reflection: string;
  }) =>
    invoke<EngineeringSessionResult>('submit_classroom_engineering_session', { input }),
  getClassroomExercise: (sessionId: number) =>
    invoke<ClassroomExerciseView | null>('get_classroom_exercise', { sessionId }),
  saveClassroomExerciseDraft: (sessionId: number, draft: string) =>
    invoke<void>('save_classroom_exercise_draft', { sessionId, draft }),
  saveClassroomExerciseCompletion: (
    sessionId: number,
    completed: boolean,
    reflection: string,
  ) =>
    invoke<void>('save_classroom_exercise_completion', {
      sessionId,
      completed,
      reflection,
    }),
  getClassroomChat: (sessionId: number) =>
    invoke<ChatMessage[]>('get_classroom_chat', { sessionId }),
  sendClassroomMessage: (sessionId: number, message: string) =>
    invoke<ChatMessage[]>('send_classroom_message', { sessionId, message }),
  submitLanguageSession: (input: {
    session_id: number;
    answers: number[];
    writing_response: string;
    speaking_completed: boolean;
    listened: boolean;
    confidence: number;
  }) => invoke<LanguageSessionResult>('submit_language_session', { input }),
  pauseSchedule: () => invoke<void>('pause_schedule'),
  resumeSchedule: () => invoke<void>('resume_schedule'),
  startSession: (focus: FocusArea) => invoke<SessionView>('start_session', { focus }),
  getQuiz: () => invoke<QuizQuestionView[]>('get_quiz'),
  submitAnswer: (questionId: number, answer: string) =>
    invoke<void>('submit_answer', { questionId, answer }),
  finishQuiz: () => invoke<ReviewData>('finish_quiz'),
  getReview: () => invoke<ReviewData>('get_review'),
  finishReview: () => invoke<SessionView>('finish_review'),
  completeTrackDay: () => invoke<SessionView>('complete_track_day'),
  getRoulette: (revisit?: boolean) => invoke<RouletteView>('get_roulette', { revisit }),
  ensureCourse: () => invoke<CourseView>('ensure_course'),
  startCourse: () => invoke<SessionView>('start_course'),
  finishCourse: () => invoke<SessionView>('finish_course'),
  escapeSession: (phrase: string) => invoke<boolean>('escape_session', { phrase }),
  extendSession: (focus: FocusArea) => invoke<SessionView>('extend_session', { focus }),
  ensureAudio: () => invoke<AudioView>('ensure_audio'),
  getAudioEnabled: () => invoke<boolean>('get_audio_enabled'),
  setAudioEnabled: (enabled: boolean) => invoke<void>('set_audio_enabled', { enabled }),
  getExitQuiz: () => invoke<ExitQuizQuestion[]>('get_exit_quiz'),
  submitExitQuiz: (answers: Record<number, string>) =>
    invoke<ExitQuizResult>('submit_exit_quiz', { answers }),
  getEscapePhrase: () => invoke<string>('get_escape_phrase'),
  getDashboard: () => invoke<DashboardView>('get_dashboard'),
  getPastCourse: (date: string) =>
    invoke<ArchivedCourse | null>('get_past_course', { date }),
  openResources: () => invoke<number>('open_resources'),
  getCourseExercise: (courseId: number) =>
    invoke<ExerciseView | null>('get_course_exercise', { courseId }),
  saveExerciseDraft: (courseId: number, draft: string) =>
    invoke<void>('save_exercise_draft', { courseId, draft }),
  saveExerciseCompletion: (
    courseId: number,
    completed: boolean,
    reflection: string,
  ) =>
    invoke<void>('save_exercise_completion', {
      courseId,
      completed,
      reflection,
    }),
  getCourseChat: (courseId: number) =>
    invoke<ChatMessage[]>('get_course_chat', { courseId }),
  sendCourseMessage: (courseId: number, message: string) =>
    invoke<ChatMessage[]>('send_course_message', { courseId, message }),
};

/** Demo mode: outside Tauri (plain `vite dev`), serve canned data from mock.ts. */
import { mockApi, mockListen } from './mock';

export const api: typeof realApi = isTauri ? realApi : (mockApi as unknown as typeof realApi);

export function onEvent<T>(name: string, handler: (payload: T) => void): Promise<UnlistenFn> {
  if (!isTauri) {
    return Promise.resolve(mockListen(name, (p) => handler(p as T)));
  }
  return listen<T>(name, (e) => handler(e.payload));
}
