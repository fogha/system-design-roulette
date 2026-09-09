<script lang="ts">
  import { tick } from 'svelte';
  import type { ClassroomSubjectId } from '../../catalog.generated';
  import ClassroomPanel from '../../components/ClassroomPanel.svelte';
  import EnrollmentSetup from './EnrollmentSetup.svelte';
  let setupCourse = $state<ClassroomSubjectId | null>(null);
  $effect(() => { setupCourse; void tick().then(() => document.getElementById('desk-content')?.scrollTo(0, 0)); });
</script>

{#if setupCourse}
  {#key setupCourse}<EnrollmentSetup courseId={setupCourse} onclose={() => (setupCourse = null)} />{/key}
{:else}
  <div class="classes-page">
    <header><div class="meta-label">CLASS REGISTRY — CURRICULA · TUTORS · PROGRESS</div><h1>Classes</h1><p>Your subjects, learning goals and next steps.</p></header>
    <ClassroomPanel mode="classes" onsetup={(id) => (setupCourse = id)} />
  </div>
{/if}
<style>
  .classes-page { padding: 28px 30px 40px; width: 100%; max-width: 1120px; margin: 0 auto; }
  header { margin-bottom: 24px; }
  h1 { font-size: 28px; margin-top: 8px; }
  header p { color: var(--muted); font-size: 13px; margin: 8px 0 0; }
  @media (max-width: 620px) { .classes-page { padding: 22px 16px; } }
</style>
