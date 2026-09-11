<script lang="ts">
  import ClassWorkspace from './ClassWorkspace.svelte';
</script>
<div class="classes-page">
  <header><div><div class="meta-label">CLASS REGISTRY</div><h1>Classes</h1></div><p>Your subjects, learning goals and next steps.</p></header>
  <ClassWorkspace />
</div>
<style>
  .classes-page { display: flex; flex-direction: column; flex: 1; min-height: 600px; min-width: 0; overflow: hidden; padding: 20px 28px 24px; }
  header { display: flex; align-items: flex-end; justify-content: space-between; gap: 16px; margin-bottom: 18px; flex-shrink: 0; } .meta-label { font-size: 9px; } h1 { font-size: 28px; margin: 6px 0 0; } p { color: var(--muted); font-size: 12px; margin: 0 0 4px; }
  @media(max-width:700px) { .classes-page { padding: 16px 14px; } header { margin-bottom: 12px; } header p { display: none; } h1 { font-size: 25px; } }
</style>
