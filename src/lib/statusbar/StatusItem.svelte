<script lang="ts">
  import type { StatusItemRef } from "./layout";
  import AgentInfoItem from "./items/AgentInfoItem.svelte";
  import AgentsRunningItem from "./items/AgentsRunningItem.svelte";
  import AgentStateItem from "./items/AgentStateItem.svelte";
  import AiModeItem from "./items/AiModeItem.svelte";
  import ClockItem from "./items/ClockItem.svelte";
  import ConsoleInfoItem from "./items/ConsoleInfoItem.svelte";
  import ContextItem from "./items/ContextItem.svelte";
  import DaemonItem from "./items/DaemonItem.svelte";
  import EngineItem from "./items/EngineItem.svelte";
  import GitInfoItem from "./items/GitInfoItem.svelte";
  import GitStatusItem from "./items/GitStatusItem.svelte";
  import MemoryItem from "./items/MemoryItem.svelte";
  import NudgeItem from "./items/NudgeItem.svelte";
  import PathItem from "./items/PathItem.svelte";
  import PlanItem from "./items/PlanItem.svelte";
  import PromptItem from "./items/PromptItem.svelte";
  import RemoteItem from "./items/RemoteItem.svelte";
  import UsageItem from "./items/UsageItem.svelte";

  /** One item of the bar, by its id. Each is its own component so that what
   * isn't in the bar isn't mounted, and asks nothing of the backend. */
  let { item }: { item: StatusItemRef } = $props();
</script>

{#if item.id === "ai.mode"}
  <AiModeItem />
{:else if item.id === "agent.engine"}
  <EngineItem />
{:else if item.id === "agent.state"}
  <AgentStateItem />
{:else if item.id === "agent.plan"}
  <PlanItem {item} />
{:else if item.id === "agent.context"}
  <ContextItem {item} />
{:else if item.id === "agent.model" || item.id === "agent.cost" || item.id === "agent.spendLimit" || item.id === "agent.lines" || item.id === "agent.duration" || item.id === "agent.version"}
  <AgentInfoItem id={item.id} />
{:else if item.id === "agents.running"}
  <AgentsRunningItem />
{:else if item.id === "companion.nudge"}
  <NudgeItem />
{:else if item.id === "git.repo"}
  <GitStatusItem {item} />
{:else if item.id === "git.lastCommit" || item.id === "git.stash" || item.id === "git.upstream" || item.id === "git.lastFetch"}
  <GitInfoItem id={item.id} />
{:else if item.id === "path.cwd"}
  <PathItem {item} />
{:else if item.id === "console.process" || item.id === "console.size"}
  <ConsoleInfoItem id={item.id} />
{:else if item.id === "console.prompt"}
  <PromptItem />
{:else if item.id === "console.memory"}
  <MemoryItem />
{:else if item.id === "console.cpu" || item.id === "system.appMemory" || item.id === "system.cpu" || item.id === "system.memory"}
  <UsageItem id={item.id} />
{:else if item.id === "system.clock"}
  <ClockItem {item} />
{:else if item.id === "system.daemon"}
  <DaemonItem />
{:else if item.id === "system.remote"}
  <RemoteItem />
{/if}
