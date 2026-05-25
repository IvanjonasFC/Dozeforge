<script lang="ts">
  import type { CulpritRanking } from '$types';
  import { formatDuration, formatScore } from '$utils/format';

  interface Props {
    culprits: CulpritRanking[];
    onSelect?: (pkg: string) => void;
  }

  let { culprits, onSelect }: Props = $props();
</script>

{#if culprits.length === 0}
  <div class="empty">No culprits detected yet. Run an audit.</div>
{:else}
  <div class="scroll-y">
    <table>
      <thead>
        <tr>
          <th>Package</th>
          <th>Wakelock</th>
          <th>Wakeups</th>
          <th>Jobs</th>
          <th>Proxy</th>
          <th>Score</th>
          <th></th>
        </tr>
      </thead>
      <tbody>
        {#each culprits as c (c.package)}
          <tr>
            <td class="mono">{c.package}</td>
            <td>{formatDuration(c.wakelock_ms)}</td>
            <td>{c.wakeup_count}</td>
            <td>{c.job_count}</td>
            <td>
              {#if c.redirected_from_proxy}
                <span class="badge ok" title="Real culprit behind {c.redirected_from_proxy}">
                  via {c.redirected_from_proxy.split('.').slice(-1)[0]}
                </span>
              {:else}
                <span class="muted">--</span>
              {/if}
            </td>
            <td><strong>{formatScore(c.score)}</strong></td>
            <td>
              {#if onSelect}
                <button onclick={() => onSelect?.(c.package)}>Restrict</button>
              {/if}
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>
{/if}
