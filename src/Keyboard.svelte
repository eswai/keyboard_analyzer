<script>
  import Key from './Key.svelte';
  export let layout;

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
    <Key legend={key.legend} size={key.size} value={key.value} count={key.count}/>
    {/each}
  </div>
  {/each}
</div>

