<script lang="ts">
  import { tick } from 'svelte';
  import type { ClassroomSubjectId } from '../../catalog.generated';
  import ClassroomPanel from '../../components/ClassroomPanel.svelte';
  import EnrollmentSetup from './EnrollmentSetup.svelte';
  import PathPreview from './PathPreview.svelte';
  import { api } from '../../ipc';
  import { app } from '../../stores.svelte';
  import type { AcceptedPath } from '../../contracts/classes';
  let setupCourse = $state<ClassroomSubjectId | null>(null);
  let accepted = $state<AcceptedPath | null>(null);
  async function showPath(id: ClassroomSubjectId) { try { accepted = await api.getClassPath(id); } catch (error) { app.error = String(error); } }
  $effect(() => { setupCourse; void tick().then(() => document.getElementById('desk-content')?.scrollTo(0, 0)); });
</script>

{#if accepted}
  <PathPreview path={accepted.recommendation} acceptedRevision={accepted.revision} onclose={() => accepted = null} onfoundations={() => { setupCourse = accepted!.recommendation.course.course_id; accepted = null; }} />
{:else if setupCourse}
  {#key setupCourse}<EnrollmentSetup courseId={setupCourse} onclose={() => (setupCourse = null)} />{/key}
{:else}
  <div class="classes-page">
    <header><div class="meta-label">CLASS REGISTRY — CURRICULA · TUTORS · PROGRESS</div><h1>Classes</h1><p>Your subjects, learning goals and next steps.</p></header>
    <ClassroomPanel mode="classes" onsetup={(id) => (setupCourse = id)} onpath={showPath} />
  </div>
{/if}
<style>
  .classes-page { padding: 28px 30px 40px; width: 100%; max-width: 1120px; margin: 0 auto; }
  header { margin-bottom: 24px; }
  h1 { font-size: 28px; margin-top: 8px; }
  header p { color: var(--muted); font-size: 13px; margin: 8px 0 0; }
  @media (max-width: 620px) { .classes-page { padding: 22px 16px; } }
</style>
