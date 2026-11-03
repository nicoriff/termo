<script lang="ts">
  import KanbanBoardDialog from "./KanbanBoardDialog.svelte";
  import KanbanTaskDialog from "./KanbanTaskDialog.svelte";
  import { kanbanStore } from "./kanban-store.svelte";

  /** The kanban dialog that's open, if any: a board's settings or a task.
   * Each reads what it opens from the store as it mounts; a new one opened
   * mounts afresh. */
  const dialog = $derived(kanbanStore.dialog);
</script>

{#key dialog}
  {#if dialog?.kind === "board"}
    <KanbanBoardDialog />
  {:else if dialog?.kind === "task"}
    <KanbanTaskDialog />
  {/if}
{/key}
