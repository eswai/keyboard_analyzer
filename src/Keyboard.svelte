<script>
  import Key from './Key.svelte';
  export let layout;
  export let designview = false;

</script>

<style>

  .keyboard {
    margin: auto;
    text-align: center;
  }

  .row {
    display: flex;
  }

</style>

<div class="keyboard">
  {#each layout.keys as row}
  <div class="row">
    {#each row as key}
    {#if designview}
    <Key legend={key.legend} size={key.size} value={key.legend.length == 0 ? -1 : (key.finger + 1) / 10} color="random" home={key.home} />
    {:else}
    <Key legend={key.legend} size={key.size} value={key.value} count={key.count} home={key.home} />
    {/if}
    {/each}
  </div>
  {/each}
</div>

