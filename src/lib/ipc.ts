import type { AcceptedPath, AcceptPath, PathSummary } from './contracts/classes';
import type { RunnerConfiguration } from './contracts/agents';
import type { AgentCall, AgentPolicy, HealthCheck, RunnerInfo, ModelCatalog, LocalStatus, LocalPull } from './contracts/agents';
import type { EnrollmentOptions, EnrollmentDraft, EnrollmentDraftId, SaveEnrollmentDraft } from './contracts/enrollment';
import type { AssessmentResponse } from './contracts/assessments';
import type { DiagnosticView, PathRecommendation } from './contracts/placement';
import type { AssessmentRoundId } from './contracts/assessments';
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

import type { CourseDefinition } from './catalog';
import type { FocusArea, LanguageId, ClassroomSubjectId } from './catalog.generated';
export type { FocusArea, LanguageId, ClassroomSubjectId } from './catalog.generated';
export type { CourseDefinition } from './catalog';

export type ClassroomKind = 'language' | 'engineering';
export type AgentId = 'claude' | 'codex' | 'cursor' | 'gemini' | 'deepseek' | 'custom' | 'anthropic' | 'openai' | 'google' | 'openrouter' | 'groq' | 'mistral' | 'ollama';
export type ModelId = string;
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
  speech_locale: string;
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
  accepted_path?: PathSummary | null;
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

export interface ScheduleConflict {
  weekday: number;
  hour: number;
  minute: number;
  subject_id: string;
  label: string;
  with_slot_id: number;
  with_subject_id: string;
  with_label: string;
  with_weekday: number;
  with_hour: number;
  with_minute: number;
  with_session_minutes: number;
}

export interface ClassroomPlanView {
  slots: PlannedSlot[];
  total_weekly_minutes: number;
  target_weekly_minutes: number;
  meets_target: boolean;
  /** Overlaps with other active classes or this class's manual times; a commit is refused while any remain. */
  conflicts: ScheduleConflict[];
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
  session_id: string | null;
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
export interface QuizRoundView {
  round_id: AssessmentRoundId | null;
  revision: number;
  questions: QuizQuestionView[];
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

export interface ProgressQuery {
  subject_id?: string | null;
  search?: string;
  status?: string | null;
  page?: number;
}
export interface ProgressEntry {
  source: 'primary' | 'classroom' | 'language';
  owner_id: string;
  date: string;
  subject_id: string;
  title: string;
  status: string;
  score: number | null;
  can_read: boolean;
}
export interface ProgressClass {
  subject_id: ClassroomSubjectId;
  label: string;
  short_code: string;
  enabled: boolean;
  progress: number;
  progress_label: string;
  completed_sessions: number;
}
export interface DashboardView {
  today: string;
  classes: ProgressClass[];
  completed_sessions: number;
  study_days: number;
  streak: number;
  activity: { date: string; completed: number }[];
  history: ProgressEntry[];
  history_total: number;
  page: number;
  page_size: number;
}
export interface ProgressLesson {
  title: string;
  date: string;
  markdown: string;
  course_id: number | null;
  classroom_session_id: number | null;
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
 * Structured exercise for a course or classroom session (exactly one owner),
 * with any autosaved draft. Reachable from the active reader and from
 * archived-course history — never gated by session completion.
 */
export interface ExerciseView {
  course_id: number | null;
  classroom_session_id: number | null;
  title: string;
  instructions: string;
  starter_code: string | null;
  deliverable: string | null;
  hints: string[];
  draft: string | null;
  completed: boolean;
  reflection: string;
}

export interface ExerciseOwner {
  course_id?: number | null;
  classroom_session_id?: number | null;
}

export interface ChatOwner {
  course_id?: number | null;
  classroom_session_id?: number | null;
}

export const isTauri =
  typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

const realApi = {
  getEnrollmentOptions: (courseId: ClassroomSubjectId) => invoke<EnrollmentOptions>('get_enrollment_options', { courseId }),
  getEnrollmentDraft: (courseId: ClassroomSubjectId) => invoke<EnrollmentDraft | null>('get_enrollment_draft', { courseId }),
  saveEnrollmentDraft: (input: SaveEnrollmentDraft) => invoke<EnrollmentDraft>('save_enrollment_draft', { input }),
  getPlacementCheck: (draftId: EnrollmentDraftId) => invoke<DiagnosticView | null>('get_placement_check', { draftId }),
  startPlacementCheck: (draftId: EnrollmentDraftId, expectedRevision: number, restart = false) => invoke<DiagnosticView>('start_placement_check', { draftId, expectedRevision, restart }),
  savePlacementResponse: (draftId: EnrollmentDraftId, roundId: AssessmentRoundId, expectedRevision: number, questionId: string, response: AssessmentResponse) => invoke<number>('save_placement_response', { draftId, roundId, expectedRevision, questionId, response }),
  submitPlacementRound: (draftId: EnrollmentDraftId, roundId: AssessmentRoundId, expectedRevision: number) => invoke<DiagnosticView>('submit_placement_round', { draftId, roundId, expectedRevision }),
  continuePlacementCheck: (draftId: EnrollmentDraftId, roundId: AssessmentRoundId) => invoke<DiagnosticView>('continue_placement_check', { draftId, roundId }),
  finishPlacementCheck: (draftId: EnrollmentDraftId, roundId: AssessmentRoundId) => invoke<DiagnosticView>('finish_placement_check', { draftId, roundId }),
  getClassPath: (courseId: ClassroomSubjectId) => invoke<AcceptedPath | null>('get_class_path', { courseId }),
  acceptClassPath: (input: AcceptPath) => invoke<AcceptedPath>('accept_class_path', { input }),
  getPathRecommendation: (draftId: EnrollmentDraftId, expectedRevision: number) => invoke<PathRecommendation>('get_path_recommendation', { draftId, expectedRevision }),
  getCatalog: () => invoke<CourseDefinition[]>('get_catalog'),
  markFrontendReady: () => invoke<void>('mark_frontend_ready'),
  getAppState: () => invoke<AppStateView>('get_app_state'),
  getRunnerConfiguration: (runner: string) => invoke<RunnerConfiguration>('get_runner_configuration', { runner }),
  saveRunnerConfiguration: (configuration: RunnerConfiguration) => invoke<RunnerConfiguration>('save_runner_configuration', { configuration }),
  getRunnerModels: (runner: string, refresh = false) => invoke<ModelCatalog>('get_runner_models', { runner, refresh }),
  setRunnerKey: (runner: string, value: string) => invoke<void>('set_runner_key', { runner, value }),
  getLocalModels: () => invoke<LocalStatus>('get_local_models'),
  getLocalPulls: () => invoke<LocalPull[]>('get_local_pulls'),
  installLocalRunner: () => invoke<string>('install_local_runner'),
  startLocalRunner: () => invoke<void>('start_local_runner'),
  pullLocalModel: (model: string) => invoke<LocalPull>('pull_local_model', { model }),
  removeLocalModel: (model: string) => invoke<void>('remove_local_model', { model }),
  selectRunner: (agent: string, model: string, customBin: string) => invoke<void>('select_runner', { agent, model, customBin }),
  getOpenrouterFreeOnly: () => invoke<boolean>('get_openrouter_free_only'),
  setOpenrouterFreeOnly: (freeOnly: boolean) => invoke<void>('set_openrouter_free_only', { freeOnly }),
  listAgentRunners: () => invoke<RunnerInfo[]>('list_agent_runners'),
  testAgentConnection: (agent?: string, customBin?: string, model?: string) => invoke<HealthCheck>('test_agent_connection', { agent, customBin, model }),
  getAgentActivity: () => invoke<AgentCall[]>('get_agent_activity'),
  getAgentPolicy: () => invoke<AgentPolicy>('get_agent_policy'),
  setAgentPolicy: (policy: AgentPolicy) => invoke<void>('set_agent_policy', { policy }),
  checkAgent: (agent?: string, customBin?: string) =>
    invoke<boolean>('check_agent', { agent, customBin }),
  completeSetup: (
    escapePhrase: string,
    kioskLevel = 'hard',
    model = 'opus',
    agent = 'claude',
    customAgentBin = ''
  ) =>
    invoke<AppStateView>('complete_setup', {
      input: {
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
  getExercise: (owner: ExerciseOwner) =>
    invoke<ExerciseView | null>('get_exercise', {
      courseId: owner.course_id ?? null,
      classroomSessionId: owner.classroom_session_id ?? null,
    }),
  saveExerciseDraft: (owner: ExerciseOwner, draft: string) =>
    invoke<void>('save_exercise_draft', {
      courseId: owner.course_id ?? null,
      classroomSessionId: owner.classroom_session_id ?? null,
      draft,
    }),
  saveExerciseCompletion: (owner: ExerciseOwner, completed: boolean, reflection: string) =>
    invoke<void>('save_exercise_completion', {
      courseId: owner.course_id ?? null,
      classroomSessionId: owner.classroom_session_id ?? null,
      completed,
      reflection,
    }),
  getChat: (owner: ChatOwner) =>
    invoke<ChatMessage[]>('get_chat', {
      courseId: owner.course_id ?? null,
      classroomSessionId: owner.classroom_session_id ?? null,
    }),
  sendChatMessage: (owner: ChatOwner, message: string) =>
    invoke<ChatMessage[]>('send_chat_message', {
      courseId: owner.course_id ?? null,
      classroomSessionId: owner.classroom_session_id ?? null,
      message,
    }),
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
  getQuiz: (sessionId: string) => invoke<QuizRoundView>('get_quiz', { sessionId }),
  submitAnswer: (sessionId: string, roundId: AssessmentRoundId, expectedRevision: number, questionId: number, answer: string, confirmed: boolean) =>
    invoke<number>('submit_answer', { sessionId, roundId, expectedRevision, questionId, answer, confirmed }),
  finishQuiz: (sessionId: string, roundId: AssessmentRoundId, expectedRevision: number) => invoke<ReviewData>('finish_quiz', { sessionId, roundId, expectedRevision }),
  getReview: (sessionId: string) => invoke<ReviewData>('get_review', { sessionId }),
  finishReview: (sessionId: string) => invoke<SessionView>('finish_review', { sessionId }),
  completeTrackDay: (sessionId: string) => invoke<SessionView>('complete_track_day', { sessionId }),
  getRoulette: (sessionId: string, revisit?: boolean) => invoke<RouletteView>('get_roulette', { sessionId, revisit }),
  ensureCourse: (sessionId: string) => invoke<CourseView>('ensure_course', { sessionId }),
  startCourse: (sessionId: string) => invoke<SessionView>('start_course', { sessionId }),
  finishCourse: (sessionId: string) => invoke<SessionView>('finish_course', { sessionId }),
  escapeSession: (phrase: string) => invoke<boolean>('escape_session', { phrase }),
  ensureAudio: (sessionId: string) => invoke<AudioView>('ensure_audio', { sessionId }),
  getAudioEnabled: () => invoke<boolean>('get_audio_enabled'),
  setAudioEnabled: (enabled: boolean) => invoke<void>('set_audio_enabled', { enabled }),
  getExitQuiz: (sessionId: string) => invoke<ExitQuizQuestion[]>('get_exit_quiz', { sessionId }),
  submitExitQuiz: (sessionId: string, answers: Record<number, string>) =>
    invoke<ExitQuizResult>('submit_exit_quiz', { sessionId, answers }),
  getEscapePhrase: () => invoke<string>('get_escape_phrase'),
  getDashboard: (query: ProgressQuery = {}) => invoke<DashboardView>('get_dashboard', { query }),
  getProgressLesson: (source: ProgressEntry['source'], ownerId: string) => invoke<ProgressLesson | null>('get_progress_lesson', { source, ownerId }),
  getPastCourse: (date: string) =>
    invoke<ArchivedCourse | null>('get_past_course', { date }),
  openResources: (sessionId: string) => invoke<number>('open_resources', { sessionId }),
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
