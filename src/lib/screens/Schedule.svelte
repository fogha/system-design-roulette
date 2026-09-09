<script lang="ts">
  import { onMount } from 'svelte';
  import { api } from '../ipc';
  import { app } from '../stores.svelte';
  import NodeCard from '../components/NodeCard.svelte';
  import FlowStage from '../components/FlowStage.svelte';
  import { Clock } from 'lucide-svelte';
  import TimePicker from '../components/TimePicker.svelte';
  import ClassroomPanel from '../components/ClassroomPanel.svelte';
  let time = $state('09:00');
  let saving = $state(false);
  let saved = $state(false);
  onMount(() => { time = `${String(app.state?.schedule_hour ?? 9).padStart(2, '0')}:${String(app.state?.schedule_minute ?? 0).padStart(2, '0')}`; });
  async function update(action: 'time' | 'pause' | 'resume') {
    if (saving) return;
    saving = true; saved = false;
    try {
      if (action === 'time') { const [hour, minute] = time.split(':').map(Number); await api.updateSchedule(hour, minute); }
      if (action === 'pause') await api.pauseSchedule();
      if (action === 'resume') await api.resumeSchedule();
      await app.refresh(); saved = true;
    } catch (error) { app.error = String(error); }
    finally { saving = false; }
  }
</script>

<div class="schedule-page">
  <header><div class="meta-label">SCHEDULER — RECURRING STUDY</div><h1>Schedule</h1><p>Set recurring study times and plan around your availability.</p></header>
  <FlowStage number="01"><section class="daily" aria-label="Daily study routine"><NodeCard Icon={Clock} name="cron-scheduler" badge={app.state?.schedule_paused ? 'paused' : 'daily'} badgeTone={app.state?.schedule_paused ? 'red' : 'amber'}><div class="meta-label">FIRE_AT — DAILY TRIGGER</div><p>Your existing daily commitment follows the selected study subject. Class appointments are managed below.</p>
    <div class="schedule-controls"><TimePicker bind:value={time} /><button class="cta mono-cta" disabled={saving} onclick={() => update('time')}>{saving ? 'Saving…' : 'Save daily time'}</button><button class="ghost mono-ghost" disabled={saving} onclick={() => update(app.state?.schedule_paused ? 'resume' : 'pause')}>{app.state?.schedule_paused ? 'Resume scheduling' : 'Pause scheduling'}</button></div>
    <p role="status">{app.state?.schedule_paused ? 'Scheduling is paused.' : saved ? 'Schedule saved.' : 'Scheduling is active.'}</p>
  </NodeCard></section></FlowStage>
  <FlowStage number="02" last><ClassroomPanel mode="schedule" /></FlowStage>
</div>
<style>
  .schedule-page { padding: 30px 30px 45px; max-width: 1040px; width: 100%; margin: 0 auto; }
  h1 { font-size: 28px; margin-top: 8px; }
  p { font-size: 13px; color: var(--muted); }
  header { margin-bottom: 25px; }
  .schedule-controls { display: flex; gap: 15px; align-items: center; flex-wrap: wrap; }
  @media (max-width: 620px) { .schedule-page { padding: 22px 16px; } }
</style>
