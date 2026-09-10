<script lang="ts">
  import { untrack } from 'svelte';
  import { api, type ClassroomProgramView, type AgentId, type ModelId, type CefrLevel, type FocusPolicy } from '../../ipc';
  import { app } from '../../stores.svelte';
  import RunnerSetup from '../runners/RunnerSetup.svelte';
  import Dropdown from '../../components/Dropdown.svelte';
  import EnforcementPicker from '../../components/EnforcementPicker.svelte';
  let { program }: { program: ClassroomProgramView } = $props();
  /** The picker speaks kiosk levels; classes store focus policies. */
  const LEVEL_BY_POLICY: Record<FocusPolicy, string> = { advisory: 'advisory', focused: 'firm', strict: 'hard' };
  const POLICY_BY_LEVEL: Record<string, FocusPolicy> = { advisory: 'advisory', firm: 'focused', hard: 'strict' };
  let level = $state(untrack(() => LEVEL_BY_POLICY[program.focus_policy]));
  let focusSaving = $state(false), focusError = $state(''), focusSaved = $state(false);
  const focusDirty = $derived(POLICY_BY_LEVEL[level] !== program.focus_policy);
  $effect(() => { const saved = LEVEL_BY_POLICY[program.focus_policy]; untrack(() => { if (!focusDirty) level = saved; }); });
  async function saveFocus() {
    if (focusSaving || !focusDirty) return;
    focusSaving = true; focusError = ''; focusSaved = false;
    try { await api.setClassFocusPolicy(program.subject_id, POLICY_BY_LEVEL[level]); await app.refresh(); focusSaved = true; }
    catch (cause) { focusError = String(cause); }
    finally { focusSaving = false; }
  }
  function values(p: ClassroomProgramView) { return { agent: p.agent as string, model: p.model as string, custom: p.custom_agent_bin, minutes: p.session_minutes, start: p.language_progress?.start_level ?? 'A1', target: p.language_progress?.target_level ?? 'A2', weekly: p.language_progress?.weekly_minutes ?? 210 }; }
  let draft = $state(untrack(() => values(program)));
  let baseline = $state(untrack(() => JSON.stringify(draft)));
  let saving = $state(false), error = $state(''), saved = $state(false);
  const dirty = $derived(JSON.stringify(draft) !== baseline);
  const changedElsewhere = $derived(JSON.stringify(values(program)) !== baseline);
  const levels = ['A1','A2','B1','B2'].map(value => ({value,label:value}));
  $effect(() => { const current = values(program); untrack(() => { if (!dirty) { draft = current; baseline = JSON.stringify(current); } }); });
  function reset() { draft = values(program); baseline = JSON.stringify(draft); error = ''; saved = false; }
  async function save(event: SubmitEvent) {
    event.preventDefault();
    if (saving) return;
    saving = true; error = ''; saved = false;
    const next = $state.snapshot(draft);
    try {
      await api.configureClassroomProgram({ subject_id: program.subject_id, enabled: program.enabled, agent: next.agent as AgentId, model: next.model as ModelId, custom_agent_bin: next.custom, session_minutes: next.minutes, start_level: program.kind === 'language' ? next.start as CefrLevel : null, target_level: program.kind === 'language' ? next.target as CefrLevel : null, weekly_minutes: program.kind === 'language' ? next.weekly : null });
      await app.refresh(); baseline = JSON.stringify(next); saved = true;
    } catch (cause) { error = String(cause); }
    finally { saving = false; }
  }
</script>
<form onsubmit={save}>
  <div class="section-intro"><h3>Class settings</h3><p>Choose the tutor and pace for {program.label}.</p></div>
  {#if changedElsewhere && dirty}<p class="notice" role="status">Saved settings changed while this draft was open. Saving will apply the values below. <button type="button" class="ghost mono-ghost" onclick={reset}>Load saved settings</button></p>{/if}
  <fieldset disabled={saving}>
    <legend class="sr-only">Tutor and pace</legend>
    <RunnerSetup bind:agent={draft.agent} bind:model={draft.model} bind:customBin={draft.custom} allowKeyEditing={false} />
    <section class="pace"><h4>Study pace</h4><div class="fields">
      <label><span>Minutes per session</span><input type="number" min="10" max="120" required bind:value={draft.minutes} /></label>
      {#if program.kind === 'language'}
        <Dropdown label="Start level" bind:value={draft.start} options={levels} />
        <Dropdown label="Target level" bind:value={draft.target} options={levels} />
        <label><span>Minutes per week</span><input type="number" min="10" max="10080" required bind:value={draft.weekly} /></label>
      {/if}
    </div></section>
  </fieldset>
  {#if error}<p class="error" role="alert">{error}</p>{/if}
  <footer><span role="status">{saving ? 'Saving…' : dirty ? 'Unsaved changes' : saved ? 'Class settings saved' : 'Settings up to date'}</span><div>{#if dirty}<button type="button" class="ghost mono-ghost" onclick={reset} disabled={saving}>Reset</button>{/if}<button type="submit" class="cta mono-cta" disabled={saving || !dirty}>{saving ? 'Saving…' : 'Save settings'}</button></div></footer>
</form>
<section class="focus" aria-labelledby="focus-heading">
  <h4 id="focus-heading">Enforcement</h4>
  <p>How the desk is held while a {program.label} lesson is active. The change applies to lessons planned after saving; a lesson already in progress keeps its policy. Break glass always works and pauses the lesson with its work kept.</p>
  <EnforcementPicker bind:value={level} />
  <div class="focus-actions"><span role="status">{focusSaving ? 'Saving…' : focusDirty ? 'Unsaved enforcement change' : focusSaved ? 'Enforcement saved' : `Current: ${program.focus_policy}`}</span><button type="button" class="cta mono-cta" disabled={focusSaving || !focusDirty} onclick={saveFocus}>Save enforcement</button></div>
  {#if focusError}<p class="error" role="alert">{focusError}</p>{/if}
</section>
<style>
  form { max-width: 1000px; margin: 0 auto; } .section-intro { margin-bottom: 20px; } h3 { margin: 0 0 6px; font: 23px var(--font-display); } p { color: var(--muted); font-size: 13px; line-height: 1.6; margin: 0; }
  fieldset { min-width: 0; padding: 0; border: 0; margin: 0; } .pace { border: 1px solid var(--node-border); border-radius: var(--radius-panel); padding: 18px; margin-top: 18px; } h4 { margin: 0 0 15px; font-size: 14px; font-weight: 500; }
  .fields { display: grid; grid-template-columns: repeat(auto-fit,minmax(150px,1fr)); gap: 16px; align-items: end; } label { display: grid; gap: 9px; font-size: 12px; color: var(--muted); } input { min-width: 0; width: 100%; max-width: 220px; padding: 11px 12px; border: 1px solid var(--node-border); background: var(--bg); color: var(--fg); font: 14px var(--font-mono); border-radius: var(--radius-control); }
  footer { position: sticky; bottom: -24px; background: var(--node-bg); border-top: 1px solid var(--node-border); padding: 16px 0; margin-top: 24px; display: flex; gap: 12px; align-items: center; justify-content: space-between; } footer span { color: var(--muted); font: 11px var(--font-mono); } footer div { display: flex; gap: 8px; }
  .notice { border-left: 2px solid var(--accent); padding: 12px; margin: 0 0 16px; background: var(--surface); } .notice button { margin-top: 8px; } .error { margin-top: 14px; color: var(--led-err); overflow-wrap: anywhere; }
  .sr-only { position: absolute; width: 1px; height: 1px; overflow: hidden; clip-path: inset(50%); }
  .focus { max-width: 1000px; margin: 28px auto 0; border: 1px solid var(--node-border); border-radius: var(--radius-panel); padding: 18px; } .focus h4 { margin-bottom: 8px; } .focus > p { margin-bottom: 4px; }
  .focus-actions { display: flex; gap: 12px; align-items: center; justify-content: space-between; margin-top: 16px; } .focus-actions span { color: var(--muted); font: 11px var(--font-mono); }
  @media(max-width:700px) { footer { flex-wrap: wrap; bottom: -16px; } }
</style>
