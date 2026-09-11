import type { AcceptedPath, AcceptPath, BridgeProposal, PathSummary, RevisePath } from './contracts/classes';
import type { RunnerConfiguration } from './contracts/agents';
import type { AgentCall, AgentPolicy, HealthCheck, RunnerInfo, ModelCatalog, LocalStatus, LocalPull } from './contracts/agents';
import type { EnrollmentOptions, EnrollmentDraft, EnrollmentDraftId, SaveEnrollmentDraft } from './contracts/enrollment';
import type { AssessmentResponse } from './contracts/assessments';
import type { DiagnosticView, PathRecommendation } from './contracts/placement';
import type { UnitChallengeView } from './contracts/challenges';
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
  /** Legacy row ID or shared-runtime `study-…` ID; see `runtime`. */
  session_id: string;
  runtime: 'legacy' | 'study';
  lifecycle: string;
  revision: number;
  checkpoint: LessonCheckpoint | null;
  check: LessonCheckView | null;
  outcome: LanguageSessionResult | null;
  language: LanguageId;
  label: string;
  native_label: string;
  level: CefrLevel;
  unit_slug: string;
  phase: number;
  phase_label: string;
  /** What this pass needs beyond its check before it counts, if anything. */
  phase_requirement: string | null;
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
  session_id: string;
  passed: boolean;
  score: number;
  corrections: LanguageCorrection[];
  level_advanced_to: CefrLevel | null;
  current_level: CefrLevel;
  progress: LanguageProgramView;
  /** Unmet phase requirement that kept this pass from counting, if any. */
  requirement?: string | null;
}

/** Enforcement chosen for a class: advisory (nudge), focused (Firm kiosk), strict (Hard kiosk). */
export type FocusPolicy = 'advisory' | 'focused' | 'strict';

/** The class session holding foreground enforcement. */
export interface FocusView {
  session_id: string;
  course_id: string;
  policy: FocusPolicy;
  /** True once the kiosk is engaged for this session. */
  locked: boolean;
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
  /** Enforcement applied to sessions planned from now on. */
  focus_policy: FocusPolicy;
  /** What the accepted route says about completion, demonstrated knowledge, review and what comes next. */
  route: RouteSummary | null;
  /** Topics whose spaced review is due today (engineering classes). */
  review_due: number;
}

export interface RouteSummary {
  revision: number;
  entry_label: string;
  required_total: number;
  required_done: number;
  coverage_total: number;
  coverage_done: number;
  /** Topics checked out with evidence (unit challenges) plus placement samples demonstrated. */
  demonstrated: number;
  /** Refreshers, accepted bridges and topics whose mastery decayed or struggles. */
  needs_review: number;
  /** The topic selection would serve next, and why. */
  next: { slug: string; title: string; reason: string } | null;
  /** The curriculum changed since acceptance; review the path before the next lesson. */
  stale: boolean;
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
  /** Status against the accepted route. */
  path_status: PathStatus;
  /** Core topic still required by the accepted route. */
  required: boolean;
}

export type PathStatus =
  | 'completed_here'
  | 'prior_knowledge_checked'
  | 'bypassed_by_choice'
  | 'needs_refresher'
  | 'not_assessed'
  | 'bridge'
  | 'in_progress'
  | 'upcoming';

/** Remaining required work on the accepted route beside full-course coverage. */
export interface PathCoverage {
  revision: number;
  entry_label: string;
  required_total: number;
  required_done: number;
  coverage_total: number;
  coverage_done: number;
  bypassed: number;
  checked: number;
  refreshers: number;
  bridges: number;
}

export interface CurriculumMapView {
  focus: FocusArea;
  label: string;
  month_outcome: string;
  completed_sessions: number;
  current_phase: CurriculumPhase;
  concepts: CurriculumConceptView[];
  path: PathCoverage | null;
  /** Gaps seen in practice that a short bridge lesson would close. */
  bridge_proposals: BridgeProposal[];
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
  /** Today's durable appointment for this rule, once materialized. */
  occurrence_id: string | null;
  disposition: string | null;
}

export type AppointmentDisposition = 'scheduled' | 'due' | 'started' | 'completed' | 'skipped' | 'missed';
export interface AppointmentView {
  id: string;
  course_id: ClassroomSubjectId;
  label: string;
  short_code: string;
  kind: ClassroomKind;
  rule_id: number | null;
  local_date: string;
  local_time: string;
  fires_at: string;
  duration_minutes: number;
  disposition: AppointmentDisposition;
  session_ref: string | null;
  /** A missed appointment can still be started as a make-up session. */
  make_up: boolean;
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
  session_id: string;
  subject_id: ClassroomSubjectId;
  kind: ClassroomKind;
  label: string;
  title: string;
  runtime: 'legacy' | 'study';
  /** Shared-runtime lifecycle, or `in_progress` for legacy rows. */
  lifecycle: string;
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

export type LessonStage = 'recall' | 'learn' | 'practice' | 'check' | 'feedback';
export interface LessonReadingPosition { anchor: string | null; offset: number }
export interface LessonCheckpoint {
  revision: number;
  body: { stage: LessonStage; reading: LessonReadingPosition; work: Record<string, unknown> };
  updated_at: string;
}
/** Saved answers for a frozen knowledge-check round. */
export interface LessonCheckView {
  round_id: AssessmentRoundId;
  revision: number;
  responses: Record<string, AssessmentResponse>;
  submitted: boolean;
}

export interface EngineeringLessonView {
  /** Legacy classroom row ID or shared-runtime `study-…` ID; see `runtime`. */
  session_id: string;
  runtime: 'legacy' | 'study';
  lifecycle: string;
  revision: number;
  checkpoint: LessonCheckpoint | null;
  check: LessonCheckView | null;
  outcome: EngineeringSessionResult | null;
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
  /** `lesson`, or `retrieval` for a delayed review without a new lesson. */
  kind: 'lesson' | 'retrieval';
  /** False when a retrieval repeats the last lesson's questions. */
  fresh_sample: boolean;
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
  session_id: string;
  subject_id: FocusArea;
  passed: boolean;
  score: number;
  corrections: ClassroomCorrection[];
  kind: 'lesson' | 'retrieval';
  fresh_sample: boolean;
}

export interface AlarmView {
  occurrence_id: string;
  course_id: ClassroomSubjectId;
  label: string;
  snoozed_until: string | null;
  queued: number;
}
export interface AppStateView {
  onboarded: boolean;
  selected_focus: FocusArea;
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
  /** The study alarm standing right now. Only starting the lesson clears it; a snooze reports its end. */
  alarm: AlarmView | null;
  classroom_programs: ClassroomProgramView[];
  classroom_slots: ClassroomSlotView[];
  classroom_due_count: number;
  active_classroom_sessions: ActiveClassroomSessionView[];
  /** Today's durable appointments plus recent missed ones awaiting make-up. */
  appointments: AppointmentView[];
  /** The class session holding foreground enforcement, if any. */
  focus: FocusView | null;
}

export interface Resource {
  title: string;
  url: string;
  type?: string;
  why?: string;
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
  source: 'primary' | 'classroom' | 'language' | 'study';
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
  study_session_id: string | null;
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

/**
 * Structured exercise for a course or classroom session (exactly one owner),
 * with any autosaved draft. Reachable from the active reader and from
 * archived-course history — never gated by session completion.
 */
export interface ExerciseView {
  course_id: number | null;
  classroom_session_id: number | null;
  study_session_id: string | null;
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
  study_session_id?: string | null;
}

export interface ChatOwner {
  course_id?: number | null;
  classroom_session_id?: number | null;
  study_session_id?: string | null;
}

export interface ClassLessonWorkInput {
  session_id: string;
  /** Omit to merge into the latest checkpoint (cooperating widgets of one open lesson). */
  expected_revision?: number | null;
  stage?: LessonStage | null;
  reading?: LessonReadingPosition | null;
  work?: Record<string, unknown>;
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
  reviseClassPath: (input: RevisePath) => invoke<AcceptedPath>('revise_class_path', { input }),
  getUnitChallenge: (courseId: ClassroomSubjectId) => invoke<UnitChallengeView | null>('get_unit_challenge', { courseId }),
  startUnitChallenge: (courseId: ClassroomSubjectId, unit: string, restart = false) => invoke<UnitChallengeView>('start_unit_challenge', { courseId, unit, restart }),
  saveUnitChallengeResponse: (courseId: ClassroomSubjectId, roundId: AssessmentRoundId, expectedRevision: number, questionId: string, response: AssessmentResponse) => invoke<number>('save_unit_challenge_response', { courseId, roundId, expectedRevision, questionId, response }),
  submitUnitChallengeRound: (courseId: ClassroomSubjectId, roundId: AssessmentRoundId, expectedRevision: number) => invoke<UnitChallengeView>('submit_unit_challenge_round', { courseId, roundId, expectedRevision }),
  applyUnitChallenge: (courseId: ClassroomSubjectId, attemptId: string, expectedRevision: number) => invoke<AcceptedPath>('apply_unit_challenge', { courseId, attemptId, expectedRevision }),
  acceptClassPath: (input: AcceptPath) => invoke<AcceptedPath>('accept_class_path', { input }),
  getPathRecommendation: (draftId: EnrollmentDraftId, expectedRevision: number) => invoke<PathRecommendation>('get_path_recommendation', { draftId, expectedRevision }),
  getCatalog: () => invoke<CourseDefinition[]>('get_catalog'),
  markFrontendReady: () => invoke<void>('mark_frontend_ready'),
  setClassFocusPolicy: (subjectId: ClassroomSubjectId, policy: FocusPolicy) =>
    invoke<ClassroomProgramView>('set_class_focus_policy', { subjectId, policy }),
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
    occurrenceId?: string | null,
  ) =>
    invoke<ClassroomSessionStart>('start_classroom_session', {
      subjectId,
      slotId: slotId ?? null,
      revisit: revisit ?? false,
      occurrenceId: occurrenceId ?? null,
    }),
  skipAppointment: (occurrenceId: string) => invoke<AppointmentView>('skip_appointment', { occurrenceId }),
  snoozeAlarm: (occurrenceId: string, minutes: number) => invoke<string>('snooze_alarm', { occurrenceId, minutes }),
  showDesk: () => invoke<void>('show_desk'),
  startFromTray: (occurrenceId: string) => invoke<void>('start_from_tray', { occurrenceId }),
  quitDesk: () => invoke<void>('quit_desk'),
  hideTrayPanel: () => invoke<void>('hide_tray_panel'),
  rescheduleAppointment: (occurrenceId: string, localTime: string) => invoke<AppointmentView>('reschedule_appointment', { occurrenceId, localTime }),
  getClassAppointments: (subjectId: ClassroomSubjectId) => invoke<AppointmentView[]>('get_class_appointments', { subjectId }),
  resumeClassroomSession: (subjectId: ClassroomSubjectId) =>
    invoke<ClassroomSessionStart | null>('resume_classroom_session', { subjectId }),
  /** Start or resume a delayed-retrieval session for the class's most overdue topic. */
  startClassReview: (subjectId: ClassroomSubjectId) => invoke<ClassroomSessionStart>('start_class_review', { subjectId }),
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
      studySessionId: owner.study_session_id ?? null,
    }),
  saveExerciseDraft: (owner: ExerciseOwner, draft: string) =>
    invoke<void>('save_exercise_draft', {
      courseId: owner.course_id ?? null,
      classroomSessionId: owner.classroom_session_id ?? null,
      studySessionId: owner.study_session_id ?? null,
      draft,
    }),
  saveExerciseCompletion: (owner: ExerciseOwner, completed: boolean, reflection: string) =>
    invoke<void>('save_exercise_completion', {
      courseId: owner.course_id ?? null,
      classroomSessionId: owner.classroom_session_id ?? null,
      studySessionId: owner.study_session_id ?? null,
      completed,
      reflection,
    }),
  getChat: (owner: ChatOwner) =>
    invoke<ChatMessage[]>('get_chat', {
      courseId: owner.course_id ?? null,
      classroomSessionId: owner.classroom_session_id ?? null,
      studySessionId: owner.study_session_id ?? null,
    }),
  sendChatMessage: (owner: ChatOwner, message: string) =>
    invoke<ChatMessage[]>('send_chat_message', {
      courseId: owner.course_id ?? null,
      classroomSessionId: owner.classroom_session_id ?? null,
      studySessionId: owner.study_session_id ?? null,
      message,
    }),
  saveClassLessonWork: (input: ClassLessonWorkInput) => invoke<LessonCheckpoint>('save_class_lesson_work', { input }),
  saveClassCheckAnswer: (sessionId: string, roundId: AssessmentRoundId, expectedRevision: number, questionId: number, choice: number | null) =>
    invoke<LessonCheckView>('save_class_check_answer', { sessionId, roundId, expectedRevision, questionId, choice }),
  submitClassCheck: (sessionId: string, roundId: AssessmentRoundId, expectedRevision: number, reflection: string) =>
    invoke<EngineeringSessionResult>('submit_class_check', { sessionId, roundId, expectedRevision, reflection }),
  submitClassLanguageCheck: (sessionId: string, roundId: AssessmentRoundId, expectedRevision: number, input: { writing_response: string; speaking_completed: boolean; listened: boolean; confidence: number }) =>
    invoke<LanguageSessionResult>('submit_class_language_check', { sessionId, roundId, expectedRevision, input }),
  pauseClassLesson: (sessionId: string) => invoke<void>('pause_class_lesson', { sessionId }),
  skipClassLesson: (sessionId: string) => invoke<void>('skip_class_lesson', { sessionId }),
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
  escapeSession: (phrase: string) => invoke<boolean>('escape_session', { phrase }),
  getEscapePhrase: () => invoke<string>('get_escape_phrase'),
  getDashboard: (query: ProgressQuery = {}) => invoke<DashboardView>('get_dashboard', { query }),
  getProgressLesson: (source: ProgressEntry['source'], ownerId: string) => invoke<ProgressLesson | null>('get_progress_lesson', { source, ownerId }),
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
