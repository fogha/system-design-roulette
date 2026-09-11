<script lang="ts">
  import '../lib/theme.css';
  import { app, shouldShowEscapeHatch } from '../lib/stores.svelte';
  import SetupWizard from '../lib/screens/SetupWizard.svelte';
  import Today from '../lib/screens/Today.svelte';
  import Classes from '../lib/features/classes/Classes.svelte';
  import StudySettings from '../lib/screens/StudySettings.svelte';
  import DeskShell from '../lib/app/DeskShell.svelte';
  import LanguageLesson from '../lib/screens/LanguageLesson.svelte';
  import ClassroomLesson from '../lib/screens/ClassroomLesson.svelte';
  import Dashboard from '../lib/screens/Dashboard.svelte';
  import EscapeHatch from '../lib/components/EscapeHatch.svelte';

  const isBlanker =
    typeof location !== 'undefined' && new URLSearchParams(location.search).has('blanker');
  const showEscapeHatch = $derived(shouldShowEscapeHatch(app.state));

  $effect(() => {
    if (!isBlanker) app.init();
  });

  // Block common quit/close shortcuts while locked.
  function onKeydown(e: KeyboardEvent) {
    if (!app.state?.debug_day && app.locked && e.metaKey && ['q', 'w', 'h', 'm'].includes(e.key.toLowerCase())) {
      e.preventDefault();
      e.stopPropagation();
    }
  }
</script>

<svelte:window onkeydown={onKeydown} oncontextmenu={(e) => !app.state?.debug_day && app.locked && e.preventDefault()} />

<div id="app-root" class="theme-noir">
  {#if isBlanker}
    <div style="flex: 1; background: #000;"></div>
  {:else if app.screen === 'loading'}
    <div class="screen"><p>…</p></div>
  {:else if app.screen === 'setup'}
    <SetupWizard />
  {:else if app.screen === 'idle' || app.screen === 'dashboard'}
    <DeskShell>
      {#if app.screen === 'dashboard'}<Dashboard />
      {:else if app.destination === 'classes'}<Classes />
      {:else if app.destination === 'settings'}<StudySettings />
      {:else}<Today />{/if}
    </DeskShell>
  {:else if app.screen === 'language'}
    <LanguageLesson />
  {:else if app.screen === 'classroom'}
    <ClassroomLesson />
  {/if}

  {#if showEscapeHatch}
    <EscapeHatch />
  {/if}

  {#if app.error}
    <div class="error-toast" role="alert">
      {app.error}
      <button onclick={() => (app.error = '')}>×</button>
    </div>
  {/if}
</div>

<style>
  .error-toast {
    position: fixed;
    bottom: 16px;
    left: 16px;
    background: var(--bad-bg);
    color: var(--bad-fg);
    border-radius: 10px;
    padding: 10px 14px;
    font-size: 13px;
    display: flex;
    gap: 12px;
    align-items: center;
    max-width: 480px;
    z-index: 60;
  }
  .error-toast button {
    background: none;
    border: none;
    color: inherit;
    font-size: 16px;
    cursor: pointer;
  }
</style>
